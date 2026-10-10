// vue-native dev (WP 3.7, part B): the edit, check, build and restart loop of the headless target.
//
// The sources of the project are polled every --poll-ms (100 by default). A change is handled once the files have been
// quiet for one poll interval (edits in progress are coalesced). Then the loop:
//   1. runs the check of `vue-native check --target native` (diagnostics, then the type checker unless --no-types);
//      a failed check keeps the running app,
//   2. builds the headless app in its dev profile, with the dev form of <script setup> (compileSfc devState); a failed
//      build keeps the running app,
//   3. asks the running app for a state snapshot (--no-snapshot skips this),
//   4. stops the running app and starts the new one with the snapshot; the new app restores what matches by name.
// Restart latency is measured from the newest modification time of the changed files to the app's mounted reply.
//
// The app protocol is JSON lines (tooling/cli/headless/main.rs, --serve). Commands go to the app's stdin, one reply
// event per command comes back on its stdout:
//   {"op":"start","restore":<snapshot>|null}  -> {"event":"mounted","restored":[...],"skipped":[{"name","reason"}]}
//                                             |  {"event":"error","kind":"app","message":...}
//   {"op":"snapshot"}                         -> {"event":"snapshot","state":{...},"dropped":[...]}
//   {"op":"steps","steps":[...]}              -> {"event":"steps","results":[...]}
//   {"op":"quit"}                             -> the app exits
// The snapshot holds the refs and reactive objects of the root's <script setup>, as tagged plain data (state.rs). Closures,
// host handles and other non-plain values are left out and named in "dropped".
//
// Events of this loop (one JSON object per line with --json; text otherwise): watching, changed, check, built, snapshot,
// ready, error (stage: check | build | snapshot | start | watch | dev), exited, output, idle (outcome: ready | kept | failed,
// once per cycle), stopped. The app's binaries alternate between gen-0 and gen-1 under
// <target dir>/vue-native-dev/<hash of the project root>: the new binary is never written over the running one.
import { spawn } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { BuildError, buildHeadlessApp, defaultName, toolEnv } from './build.mjs';
import { checkProject } from './commands.mjs';
import { UsageError, diagnosticJson, formatDiagnostics, resolveProject } from './project.mjs';

export const DEFAULT_POLL_MS = 100;
const WATCHED_EXTENSIONS = new Set(['.vue', '.js', '.mjs', '.ts', '.mts', '.css', '.html']);
const START_TIMEOUT_MS = 120000;
const REPLY_TIMEOUT_MS = 30000;
const QUIT_GRACE_MS = 2000;

// The watched files of a project: relative path ('/'-separated) -> { mtimeMs, size }. node_modules, dot directories and
// the paths in `skip` are not scanned.
export function scanSources(root, skip = []) {
  const found = new Map();
  const walk = (dir) => {
    for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, ent.name);
      if (ent.name.startsWith('.') || ent.name === 'node_modules') continue;
      if (skip.some((s) => full === s || full.startsWith(`${s}${path.sep}`))) continue;
      if (ent.isDirectory()) {
        walk(full);
      } else if (ent.isFile() && WATCHED_EXTENSIONS.has(path.extname(ent.name))) {
        let st;
        try {
          st = fs.statSync(full);
        } catch {
          continue; // removed since the directory was read
        }
        found.set(path.relative(root, full).split(path.sep).join('/'), { mtimeMs: st.mtimeMs, size: st.size });
      }
    }
  };
  walk(root);
  return found;
}

// The files that differ between two scans (changed, added or removed), sorted.
export function changedFiles(before, after) {
  const out = [];
  for (const [file, sig] of after) {
    const prev = before.get(file);
    if (prev === undefined || prev.mtimeMs !== sig.mtimeMs || prev.size !== sig.size) out.push(file);
  }
  for (const file of before.keys()) {
    if (!after.has(file)) out.push(file);
  }
  return out.sort();
}

// The text form of an event: [stream, line] pairs. Warnings and errors go to stderr.
function renderText(e) {
  switch (e.event) {
    case 'watching':
      return [['out', `vue-native dev: watching ${e.files} source file(s) under ${e.root} (entry ${e.entry}); Ctrl-C stops`]];
    case 'changed':
      return [['out', `changed: ${e.files.join(', ')}`]];
    case 'check':
      return e.ok
        ? [['out', `check: ok (${e.warnings} warning(s), types ${e.typesChecked ? 'checked' : 'not checked'}, ${e.ms} ms)`]]
        : [['out', `check: failed (${e.errors} error(s))`]];
    case 'built':
      return [['out', `built ${e.name}: compile ${e.compileMs} ms, cargo ${e.cargoMs} ms`]];
    case 'snapshot':
      if (!e.enabled) return [['out', 'state: not kept (--no-snapshot)']];
      return [['out', `state: saved ${e.bindings} binding(s)`], ...e.dropped.map((d) => ['err', `warning: not saved: ${d}`])];
    case 'ready': {
      const edit = e.editToReadyMs === null ? '' : `, ${e.editToReadyMs} ms after the edit`;
      const restored = e.restored.length > 0 ? `; restored: ${e.restored.join(', ')}` : '';
      return [
        ['out', `ready: app mounted in ${e.startMs} ms${edit}${restored}`],
        ...e.skipped.map((s) => ['err', `warning: not restored: ${s.name}: ${s.reason}`]),
      ];
    }
    case 'error':
      return [['err', `error (${e.stage}): ${e.message}`]];
    case 'exited':
      return [['err', `warning: the app process ended (${e.code === null ? `signal ${e.signal}` : `code ${e.code}`})`]];
    case 'idle':
      return [['out', 'waiting for changes']];
    case 'stopped':
      return [['out', 'vue-native dev: stopped']];
    case 'output':
      return [[e.stream === 'stderr' ? 'err' : 'out', `app ${e.stream}: ${e.text}`]];
    default:
      return [];
  }
}

class DevSession {
  constructor({ root, entry, types, snapshot, json, pollMs, io, name }) {
    this.root = root;
    this.entry = entry;
    this.types = types;
    this.snapshot = snapshot;
    this.json = json;
    this.pollMs = pollMs;
    this.io = io;
    this.name = name;
    this.outBase = path.join(path.resolve(toolEnv().CARGO_TARGET_DIR), 'vue-native-dev', crypto.createHash('sha256').update(root).digest('hex').slice(0, 16));
    this.skip = [this.outBase];
    this.events = [];
    this.waiters = [];
    this.child = null;      // the running app process
    this.childDir = null;   // the build directory of the running app
    this.waiting = null;    // the resolver of the reply being awaited
    this.pending = new Map(); // changed file -> modification time, until the reload that handles it
    this.signature = new Map();
    this.timer = null;
    this.stopped = false;
    this.started = this.initial();
  }

  emit(event) {
    this.events.push(event);
    if (this.json) {
      this.io.out.write(`${JSON.stringify(event)}\n`);
    } else {
      for (const [stream, line] of renderText(event)) this.io[stream === 'err' ? 'err' : 'out'].write(`${line}\n`);
    }
    for (const w of this.waiters.filter((x) => x.pred(event))) {
      this.waiters = this.waiters.filter((x) => x !== w);
      w.done(event);
    }
  }

  // Resolves with the first event (after this call) that satisfies pred; rejects after timeoutMs. The default is five
  // minutes: a cycle can queue behind another build for the shared cargo target lock (a release build takes minutes).
  waitFor(pred, timeoutMs = 300000) {
    return new Promise((resolve, reject) => {
      const waiter = { pred, done: (ev) => { clearTimeout(timer); resolve(ev); } };
      const timer = setTimeout(() => {
        this.waiters = this.waiters.filter((x) => x !== waiter);
        reject(new Error(`no matching event within ${timeoutMs} ms`));
      }, timeoutMs);
      this.waiters.push(waiter);
    });
  }

  // Resolves at the end of the next cycle of the loop (its idle event).
  nextIdle(timeoutMs) {
    return this.waitFor((e) => e.event === 'idle', timeoutMs);
  }

  async initial() {
    this.signature = scanSources(this.root, this.skip);
    this.emit({ event: 'watching', files: this.signature.size, root: this.root, entry: this.entry });
    const built = this.build();
    const outcome = built === null ? 'failed' : await this.launch(built, null, undefined);
    this.emit({ event: 'idle', outcome });
    this.schedule();
  }

  schedule() {
    if (!this.stopped) this.timer = setTimeout(() => this.poll(), this.pollMs);
  }

  async poll() {
    if (this.stopped) return;
    let now;
    try {
      now = scanSources(this.root, this.skip);
    } catch (err) {
      this.emit({ event: 'error', stage: 'watch', message: err.message });
      this.schedule();
      return;
    }
    const changed = changedFiles(this.signature, now);
    this.signature = now;
    if (changed.length > 0) {
      for (const file of changed) this.pending.set(file, now.get(file)?.mtimeMs ?? Date.now());
      this.schedule();
      return;
    }
    if (this.pending.size > 0) {
      const files = [...this.pending.keys()].sort();
      const editAt = Math.max(...this.pending.values());
      this.pending = new Map();
      await this.reload(files, editAt);
    }
    this.schedule();
  }

  async reload(files, editAt) {
    this.emit({ event: 'changed', files });
    let outcome;
    try {
      outcome = await this.rebuild(editAt);
    } catch (err) {
      this.emit({ event: 'error', stage: 'dev', message: err.message });
      outcome = this.child === null ? 'failed' : 'kept';
    }
    this.emit({ event: 'idle', outcome });
  }

  // One cycle after a change: check and build (the running app stays until the new build exists), then snapshot, stop,
  // start. Returns the outcome: 'ready' (the new app is mounted), 'kept' (the running app stays) or 'failed'.
  async rebuild(editAt) {
    const built = this.build();
    if (built === null) return this.child === null ? 'failed' : 'kept';
    if (this.stopped) return 'kept';
    let state = null;
    if (this.child !== null) {
      if (this.snapshot) {
        state = await this.takeSnapshot();
      } else {
        this.emit({ event: 'snapshot', enabled: false });
      }
      await this.stopChild();
    }
    return this.launch(built, state, editAt);
  }

  // The check and the build of the headless app. Returns { dir, binary } or null (the failure is reported).
  build() {
    const checkStart = Date.now();
    const c = checkProject({ root: this.root, entry: this.entry, target: 'native', types: this.types });
    const checkMs = Date.now() - checkStart;
    if (c.diagnostics.length > 0 && !this.json) {
      this.io.err.write(`${formatDiagnostics(c.diagnostics, c.compiled.sourceOf)}\n\n`);
    }
    this.emit({
      event: 'check', ok: c.ok, errors: c.errors, warnings: c.warnings, typesChecked: c.typeCheck.ran, ms: checkMs,
      diagnostics: c.diagnostics.map(diagnosticJson),
    });
    if (!c.ok) return null;
    let built;
    try {
      built = buildHeadlessApp({ root: this.root, entry: this.entry, out: this.nextDir(), name: this.name, profile: 'dev', devState: true });
    } catch (err) {
      if (!(err instanceof BuildError)) throw err;
      this.emit({ event: 'error', stage: 'build', message: err.message });
      return null;
    }
    this.emit({
      event: 'built', name: this.name, bytes: built.manifest.bytes, checkMs,
      compileMs: built.timing.compileMs, cargoMs: built.timing.cargoMs,
    });
    return { dir: built.dir, binary: path.join(built.dir, this.name) };
  }

  // The build directory that the running app is not in.
  nextDir() {
    const gen0 = path.join(this.outBase, 'gen-0');
    const gen1 = path.join(this.outBase, 'gen-1');
    return this.childDir === gen0 ? gen1 : gen0;
  }

  // Starts the built app with a snapshot (or null). Returns 'ready' when it has mounted, else 'failed'.
  async launch(built, state, editAt) {
    if (this.stopped) return 'failed';
    const spawnAt = Date.now();
    this.childDir = built.dir;
    this.attach(spawn(built.binary, ['--serve'], { stdio: ['pipe', 'pipe', 'pipe'] }));
    const reply = await this.request({ op: 'start', restore: state }, START_TIMEOUT_MS);
    if (reply.event !== 'mounted') {
      this.emit({ event: 'error', stage: 'start', message: reply.message ?? `unexpected reply ${reply.event}` });
      return 'failed';
    }
    const now = Date.now();
    this.emit({
      event: 'ready', startMs: now - spawnAt, editToReadyMs: editAt === undefined ? null : Math.round(now - editAt),
      restored: reply.restored, skipped: reply.skipped,
    });
    return 'ready';
  }

  async takeSnapshot() {
    const reply = await this.request({ op: 'snapshot' }, REPLY_TIMEOUT_MS);
    if (reply.event !== 'snapshot') {
      this.emit({ event: 'error', stage: 'snapshot', message: `${reply.message ?? reply.event}; the app restarts without its state` });
      return null;
    }
    this.emit({ event: 'snapshot', enabled: true, bindings: Object.keys(reply.state).length, dropped: reply.dropped, state: reply.state });
    return reply.state;
  }

  attach(child) {
    this.child = child;
    let buf = '';
    child.stdout.setEncoding('utf8');
    child.stdout.on('data', (chunk) => {
      buf += chunk;
      for (let nl = buf.indexOf('\n'); nl >= 0; nl = buf.indexOf('\n')) {
        const line = buf.slice(0, nl).trim();
        buf = buf.slice(nl + 1);
        if (line !== '') this.onLine(line);
      }
    });
    child.stderr.setEncoding('utf8');
    child.stderr.on('data', (chunk) => this.emit({ event: 'output', stream: 'stderr', text: chunk.trimEnd() }));
    // A command written to an exited app fails on its stdin; the exit is reported by the exit handler.
    child.stdin.on('error', () => {});
    child.on('error', (err) => this.gone(child, { code: null, signal: null, message: err.message }));
    child.on('exit', (code, signal) => this.gone(child, { code, signal, message: `the app process ended (${code === null ? `signal ${signal}` : `code ${code}`})` }));
  }

  onLine(line) {
    let reply = null;
    try {
      reply = JSON.parse(line);
    } catch {
      // not a protocol line
    }
    if (reply !== null && this.waiting !== null) {
      const w = this.waiting;
      this.waiting = null;
      w(reply);
      return;
    }
    this.emit({ event: 'output', stream: 'stdout', text: line });
  }

  // The app process has ended. An exit that stopChild expects has already cleared this.child.
  gone(child, info) {
    if (this.child !== child) return;
    this.child = null;
    if (this.waiting !== null) {
      const w = this.waiting;
      this.waiting = null;
      w({ event: 'exited', message: info.message });
    }
    this.emit({ event: 'exited', code: info.code ?? null, signal: info.signal ?? null });
  }

  // Sends one command and resolves with its reply event (or an 'exited' or 'error' event).
  request(cmd, timeoutMs) {
    const child = this.child;
    if (child === null) return Promise.resolve({ event: 'exited', message: 'the app is not running' });
    if (this.waiting !== null) return Promise.reject(new Error('a request to the app is already in progress'));
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        this.waiting = null;
        child.kill('SIGKILL');
        resolve({ event: 'error', message: `no reply to ${cmd.op} within ${timeoutMs} ms` });
      }, timeoutMs);
      this.waiting = (reply) => {
        clearTimeout(timer);
        resolve(reply);
      };
      child.stdin.write(`${JSON.stringify(cmd)}\n`);
    });
  }

  // Stops the running app: quit, then kill after a grace period.
  async stopChild() {
    const child = this.child;
    if (child === null) return;
    this.child = null;
    if (child.exitCode !== null || child.signalCode !== null) return;
    const exited = new Promise((resolve) => child.once('exit', resolve));
    child.stdin.end('{"op":"quit"}\n');
    const timer = setTimeout(() => child.kill('SIGKILL'), QUIT_GRACE_MS);
    await exited;
    clearTimeout(timer);
  }

  // Requests a steps command from the running app (used by tests and by callers that drive the app).
  async steps(steps) {
    const reply = await this.request({ op: 'steps', steps }, REPLY_TIMEOUT_MS);
    if (reply.event !== 'steps') throw new Error(reply.message ?? `unexpected reply ${reply.event}`);
    return reply.results;
  }

  async stop() {
    if (this.stopped) return;
    this.stopped = true;
    clearTimeout(this.timer);
    await this.stopChild();
    this.emit({ event: 'stopped' });
  }
}

// Starts a dev session for a resolved project ({ root, entry }). The session's `started` promise settles after the first
// cycle. Tests and the CLI use the same session.
export function startDev({ root, entry, types = true, snapshot = true, json = false, pollMs = DEFAULT_POLL_MS, io, name = defaultName(root) }) {
  return new DevSession({ root, entry, types, snapshot, json, pollMs, io, name });
}

function parsePollMs(text) {
  if (text === undefined) return DEFAULT_POLL_MS;
  const n = Number(text);
  if (!Number.isInteger(n) || n < 10) throw new UsageError(`--poll-ms must be an integer of at least 10, not "${text}"`);
  return n;
}

// vue-native dev <entry.vue|dir> [--root <dir>] [--no-types] [--no-snapshot] [--json] [--poll-ms <n>] [--name <name>]
// Runs until interrupted (Ctrl-C), then stops the app and returns 0. Usage and resolution errors return 2 or 1.
export async function dev(positionals, values, io) {
  try {
    if (positionals.length !== 1) throw new UsageError('dev takes one entry .vue file or project directory');
    const pollMs = parsePollMs(values['poll-ms']);
    const { root, entry } = resolveProject(positionals[0], { root: values.root });
    const session = startDev({
      root, entry, types: !values['no-types'], snapshot: !values['no-snapshot'], json: values.json === true, pollMs, io,
      name: values.name ?? defaultName(root),
    });
    await session.started;
    return await new Promise((resolve) => {
      const stop = () => {
        session.stop().then(() => resolve(0));
      };
      process.once('SIGINT', stop);
      process.once('SIGTERM', stop);
    });
  } catch (err) {
    io.err.write(`vue-native: ${err.message}\n`);
    return err instanceof UsageError ? 2 : 1;
  }
}
