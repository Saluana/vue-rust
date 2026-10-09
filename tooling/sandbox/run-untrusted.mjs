#!/usr/bin/env node
// Runs an untrusted command under macOS sandbox-exec with a generated deny-default SBPL profile (WP-G0C,
// R22.AC1). Best effort: Apple marks sandbox-exec DEPRECATED (man sandbox-exec). Scope and limits are in
// docs/security/build-trust.md. No dependencies.
//
// CLI:  node tooling/sandbox/run-untrusted.mjs --workspace DIR [--scratch DIR] [--read PATH]... [--env NAME=VALUE]...
//         [--cwd DIR] [--timeout-ms N] [--sandbox-exec PATH] [--print-profile] --cmd CMD [ARGS...]
// Exit codes: child status (128+N if killed by signal N); 2 usage; 70 internal error; 97 sandbox-exec unavailable
// (nothing ran); 98 sandbox self-test failed or could not run (nothing ran); 124 timed out.
import { spawn, spawnSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const SANDBOX_EXEC = '/usr/bin/sandbox-exec';
export const EXIT = Object.freeze({ USAGE: 2, INTERNAL: 70, TIMEOUT: 124, UNAVAILABLE: 97, INEFFECTIVE: 98 });
// Read-only trees a command needs to start (dyld, frameworks, core utilities).
export const SYSTEM_READ_PATHS = Object.freeze(['/usr/lib', '/System/Library', '/bin', '/usr/bin']);
const SYSTEM_EXEC_PATHS = Object.freeze(['/bin', '/usr/bin']);
// Home-relative directories that usually hold credentials. A workspace or read path may not overlap them.
const SECRET_HOME_DIRS = Object.freeze(['.ssh', '.gnupg', '.aws', '.cargo', '.config', 'Library/Keychains']);
const MIN_PATH = '/usr/bin:/bin:/usr/sbin:/sbin';
// Realpaths that a writable tree (workspace, scratch) must not contain.
const SYSTEM_ROOTS = Object.freeze([...SYSTEM_READ_PATHS, '/tmp'].map((p) => safeRealpath(p) ?? p));
const MAX_CAPTURE = 4 * 1024 * 1024;

export class SandboxError extends Error {
  constructor(code, message, exitCode) {
    super(message);
    this.name = 'SandboxError';
    this.code = code;
    this.exitCode = exitCode;
  }
}
const usage = (message) => new SandboxError('USAGE', message, EXIT.USAGE);

// Reports whether sandbox-exec can be used on this machine. Runs nothing.
export function sandboxAvailability({ sandboxExec = SANDBOX_EXEC } = {}) {
  const fail = (reason) => ({ available: false, path: sandboxExec, reason });
  if (process.platform !== 'darwin') return fail(`platform is ${process.platform}, not darwin`);
  if (path.basename(sandboxExec) !== 'sandbox-exec') return fail('binary is not named sandbox-exec');
  try {
    if (!fs.statSync(sandboxExec).isFile()) return fail('not a regular file');
    fs.accessSync(sandboxExec, fs.constants.X_OK);
  } catch (e) {
    return fail(`${e.code ?? e.message} at ${sandboxExec}`);
  }
  return { available: true, path: sandboxExec, reason: 'ok' };
}

// SBPL string literal. Control characters are rejected so a path cannot inject profile rules.
function sbpl(p) {
  if (/[\u0000-\u001f\u007f]/.test(p)) throw usage(`path contains control characters: ${JSON.stringify(p)}`);
  return `"${p.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}

function safeRealpath(p) {
  try {
    return fs.realpathSync(p);
  } catch {
    return null;
  }
}

// Canonical (symlink-free) path of an existing directory, or of any existing path when file is true.
// Refuses trees that contain the home or temp directory, and trees that overlap credential directories.
function tree(p, what, { file = false, guard = true, writable = false } = {}) {
  if (typeof p !== 'string' || !path.isAbsolute(p)) throw usage(`${what} must be an absolute path: ${String(p)}`);
  const real = safeRealpath(p);
  if (real === null) throw usage(`${what} does not exist: ${p}`);
  if (!file && !fs.statSync(real).isDirectory()) throw usage(`${what} is not a directory: ${p}`);
  if (!guard) return real;
  const home = fs.realpathSync(os.homedir());
  const tmp = fs.realpathSync(os.tmpdir());
  const contains = (outer, inner) => inner === outer || inner.startsWith(outer + '/');
  if (real === '/' || contains(real, home) || contains(real, tmp)) {
    throw usage(`${what} must not be /, or contain the home or temp directory: ${real}`);
  }
  for (const rel of SECRET_HOME_DIRS) {
    const secret = safeRealpath(path.join(home, rel));
    if (secret && (contains(secret, real) || contains(real, secret))) {
      throw usage(`${what} overlaps a credential directory (~/${rel}): ${real}`);
    }
  }
  if (writable) {
    // A writable tree must not contain system trees or the shared temp directory. Read-only trees may.
    for (const root of SYSTEM_ROOTS) {
      if (contains(real, root)) throw usage(`${what} must not contain the system or shared temp directory ${root}: ${real}`);
    }
  }
  return real;
}

function ancestorsOf(p) {
  const out = [];
  for (let cur = path.dirname(p); ; cur = path.dirname(cur)) {
    out.push(cur);
    if (cur === '/') return out;
  }
}

function isExecutable(f) {
  try {
    fs.accessSync(f, fs.constants.X_OK);
    return fs.statSync(f).isFile();
  } catch {
    return false;
  }
}

// Absolute, symlink-free path of the program to run. Bare names are searched on searchPath.
export function resolveCommand(cmd, cwd, searchPath = process.env.PATH ?? '') {
  const found = cmd.includes('/')
    ? path.resolve(cwd, cmd)
    : searchPath.split(':').filter(Boolean).map((d) => path.join(d, cmd)).find(isExecutable);
  if (!found || !isExecutable(found)) throw usage(`command not found or not executable: ${cmd}`);
  return fs.realpathSync(found);
}

// The deny-default profile. Writes: workspace and scratch only. Reads: system trees plus readPaths. Network: none.
export function buildProfile({ workspace, scratch, command, readPaths = [] }) {
  const ws = tree(workspace, 'workspace', { writable: true });
  const scr = tree(scratch, 'scratch', { writable: true });
  const cmd = tree(command, 'command', { file: true, guard: false });
  const extra = readPaths.map((p) => tree(p, 'read path', { file: true }));
  const writable = [ws, scr];
  const readable = [...SYSTEM_READ_PATHS, ...extra];
  const executable = [...SYSTEM_EXEC_PATHS, ...writable];
  const metaAncestors = [...new Set([...writable, ...readable].flatMap(ancestorsOf))];
  const lit = (ps) => ps.map((p) => `(literal ${sbpl(p)})`).join(' ');
  const sub = (ps) => ps.map((p) => `(subpath ${sbpl(p)})`).join(' ');
  return [
    '(version 1)',
    '(deny default)',
    '; No network of any kind: outbound, inbound, bind, DNS, unix sockets.',
    '(deny network*)',
    '(allow sysctl-read)',
    '(allow process-fork)',
    '; Runtime startup reads "/" (top-level names only, not file contents).',
    '(allow file-read-data (literal "/"))',
    '; stat on the directories above allowed trees (names and contents of those directories stay unreadable).',
    `(allow file-read-metadata ${lit(metaAncestors)})`,
    `(allow file-read* file-map-executable ${sub(readable)} ${lit([cmd])})`,
    '; Writes and program execution are limited to the workspace and scratch directory.',
    `(allow file-read* file-write* file-map-executable ${sub(writable)})`,
    `(allow process-exec ${lit([cmd])} ${sub(executable)})`,
    '(allow file-read* (literal "/dev/random") (literal "/dev/urandom"))',
    '(allow file-read* file-write* (literal "/dev/null"))',
    '',
  ].join('\n');
}

// Proves the mechanism is in force on this machine before untrusted code runs: a read and a write of a file
// outside every allowed tree must both fail, and the probe must actually have executed.
export function assertSandboxEffective({ sandboxExec = SANDBOX_EXEC, workspace, scratch }) {
  const probeRoot = fs.mkdtempSync(path.join(fs.realpathSync(os.tmpdir()), 'vn-sbx-probe-'));
  const fail = (code, message) => new SandboxError(code, `sandbox self-test: ${message}; nothing was run`, EXIT.INEFFECTIVE);
  try {
    const secret = path.join(probeRoot, 'secret');
    const token = `probe-${randomBytes(8).toString('hex')}`;
    fs.writeFileSync(secret, token);
    const written = path.join(probeRoot, 'written');
    const probe = (command, script, arg) =>
      spawnSync(sandboxExec, ['-p', buildProfile({ workspace, scratch, command }), command, '-c', script, 'sh', arg], {
        cwd: workspace,
        env: { PATH: MIN_PATH },
        timeout: 20000,
        encoding: 'utf8',
      });
    const read = probe('/bin/sh', 'echo PROBE_RAN; cat "$1"; echo "STATUS=$?"', secret);
    if (!read.stdout?.includes('PROBE_RAN')) throw fail('UNVERIFIED', `read probe did not run (${read.stderr?.trim() || read.error?.message || 'no output'})`);
    if (read.stdout.includes(token) || /STATUS=0/.test(read.stdout)) throw fail('INEFFECTIVE', `read of ${secret} succeeded`);
    const write = probe('/bin/sh', 'echo PROBE_RAN; printf x > "$1"; echo "STATUS=$?"', written);
    if (!write.stdout?.includes('PROBE_RAN')) throw fail('UNVERIFIED', `write probe did not run (${write.stderr?.trim() || 'no output'})`);
    if (/STATUS=0/.test(write.stdout) || fs.existsSync(written)) throw fail('INEFFECTIVE', `write of ${written} succeeded`);
  } finally {
    fs.rmSync(probeRoot, { recursive: true, force: true });
  }
}

// Validates options and computes what would run. Used by runUntrusted and by --print-profile.
export function plan({ workspace, scratch, cmd, cwd, readPaths = [] }) {
  if (typeof cmd !== 'string' || cmd === '') throw usage('cmd is required');
  const ws = tree(workspace, 'workspace', { writable: true });
  const scr = scratch === undefined ? mkdirTree(path.join(ws, 'tmp')) : tree(scratch, 'scratch', { writable: true });
  const workdir = cwd === undefined ? ws : tree(cwd, 'cwd');
  if (workdir !== ws && !workdir.startsWith(ws + '/')) throw usage(`cwd must be inside the workspace: ${workdir}`);
  const command = resolveCommand(cmd, workdir);
  const profile = buildProfile({ workspace: ws, scratch: scr, command, readPaths });
  return { workspace: ws, scratch: scr, cwd: workdir, command, profile };
}

function mkdirTree(p) {
  fs.mkdirSync(p, { recursive: true });
  return fs.realpathSync(p);
}

// Runs cmd with args inside the sandbox. Throws SandboxError (never runs anything) when the sandbox is missing or
// the self-test fails. The child gets a minimal environment: HOME and TMPDIR point into the workspace, and nothing
// is inherited from the caller except what appears in env.
export async function runUntrusted({
  workspace,
  scratch,
  cmd,
  args = [],
  env = {},
  cwd,
  readPaths = [],
  sandboxExec = SANDBOX_EXEC,
  timeoutMs = 60000,
  selfTest = true,
  forward,
} = {}) {
  const avail = sandboxAvailability({ sandboxExec });
  if (!avail.available) {
    throw new SandboxError('UNAVAILABLE', `sandbox-exec unavailable (${avail.reason}); refusing to run unsandboxed`, EXIT.UNAVAILABLE);
  }
  if (!Array.isArray(args) || args.some((a) => typeof a !== 'string')) throw usage('args must be strings');
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs <= 0) throw usage('timeoutMs must be a positive integer');
  const p = plan({ workspace, scratch, cmd, cwd, readPaths });
  if (selfTest) assertSandboxEffective({ sandboxExec, workspace: p.workspace, scratch: p.scratch });
  const childEnv = { PATH: MIN_PATH, HOME: p.workspace, TMPDIR: p.scratch, ...env };
  return runChild(sandboxExec, ['-p', p.profile, p.command, ...args], { cwd: p.cwd, env: childEnv, timeoutMs, forward });
}

// The child's stdout and stderr are always pipes, never the caller's descriptors: under sandbox-exec, Node aborts at
// startup when its stdout or stderr is a regular file outside the workspace (checked on this machine).
// The child leads its own process group. When the child exits, on timeout, and on SIGINT/SIGTERM/SIGHUP to this
// process, the group is killed so that processes the command started do not outlive the run. A process that leaves
// the group and keeps the pipes open cannot hang the runner: output is cut off after OUTPUT_GRACE_MS.
const RELAYED_SIGNALS = ['SIGINT', 'SIGTERM', 'SIGHUP'];
const OUTPUT_GRACE_MS = 2000;
function runChild(file, argv, { cwd, env, timeoutMs, forward }) {
  return new Promise((resolve, reject) => {
    const child = spawn(file, argv, { cwd, env, stdio: ['ignore', 'pipe', 'pipe'], detached: true });
    const killGroup = () => {
      try {
        process.kill(-child.pid, 'SIGKILL');
      } catch {
        // The group is already gone.
      }
    };
    const relay = (sig) => {
      cleanup();
      killGroup();
      process.kill(process.pid, sig);
    };
    const cleanup = () => {
      for (const sig of RELAYED_SIGNALS) process.off(sig, relay);
    };
    for (const sig of RELAYED_SIGNALS) process.on(sig, relay);
    const cap = { stdout: '', stderr: '', truncated: false };
    for (const name of ['stdout', 'stderr']) {
      child[name].setEncoding('utf8').on('data', (chunk) => {
        if (forward) forward[name].write(chunk);
        if (cap[name].length < MAX_CAPTURE) cap[name] += chunk;
        else cap.truncated = true;
      });
    }
    let timedOut = false;
    let exit = null;
    let settled = false;
    let timer;
    let grace;
    const finish = () => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      clearTimeout(grace);
      cleanup();
      resolve({ exitCode: exit.exitCode, signal: exit.signal, timedOut, sandboxed: true, ...cap });
    };
    timer = setTimeout(() => {
      timedOut = true;
      killGroup();
    }, timeoutMs);
    child.on('error', (e) => {
      clearTimeout(timer);
      cleanup();
      settled = true;
      reject(e);
    });
    child.on('exit', (exitCode, signal) => {
      exit = { exitCode, signal };
      clearTimeout(timer);
      killGroup();
      grace = setTimeout(() => {
        child.stdout.destroy();
        child.stderr.destroy();
        finish();
      }, OUTPUT_GRACE_MS);
    });
    child.on('close', (exitCode, signal) => {
      exit ??= { exitCode, signal };
      finish();
    });
  });
}

function parseArgs(argv) {
  const o = { readPaths: [], env: {}, args: [], timeoutMs: 60000, sandboxExec: SANDBOX_EXEC, printProfile: false, help: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const value = () => {
      if (i + 1 >= argv.length) throw usage(`${a} needs a value`);
      return argv[++i];
    };
    switch (a) {
      case '--workspace': o.workspace = value(); break;
      case '--scratch': o.scratch = value(); break;
      case '--cwd': o.cwd = value(); break;
      case '--read': o.readPaths.push(value()); break;
      case '--sandbox-exec': o.sandboxExec = value(); break;
      case '--timeout-ms': {
        const n = Number(value());
        if (!Number.isSafeInteger(n) || n <= 0) throw usage('--timeout-ms expects a positive integer');
        o.timeoutMs = n;
        break;
      }
      case '--env': {
        const kv = value();
        const eq = kv.indexOf('=');
        if (eq <= 0) throw usage('--env expects NAME=VALUE');
        o.env[kv.slice(0, eq)] = kv.slice(eq + 1);
        break;
      }
      case '--print-profile': o.printProfile = true; break;
      case '-h':
      case '--help': o.help = true; break;
      case '--cmd':
        if (i + 1 >= argv.length) throw usage('--cmd needs a command');
        o.cmd = argv[i + 1];
        o.args = argv.slice(i + 2);
        i = argv.length;
        break;
      default: throw usage(`unknown argument: ${a}`);
    }
  }
  if (o.help) return o;
  if (!o.workspace) throw usage('--workspace DIR is required');
  if (o.cmd === undefined) throw usage('--cmd CMD [ARGS...] is required (must come last)');
  return o;
}

const USAGE_TEXT = 'usage: run-untrusted.mjs --workspace DIR [--scratch DIR] [--cwd DIR] [--read PATH]... [--env K=V]... [--timeout-ms N] [--sandbox-exec PATH] [--print-profile] --cmd CMD [ARGS...]';

// Returns the process exit code. io allows tests to capture output.
export async function main(argv, io = { stdout: process.stdout, stderr: process.stderr }) {
  try {
    const o = parseArgs(argv);
    if (o.help) {
      io.stdout.write(`${USAGE_TEXT}\n`);
      return 0;
    }
    if (o.printProfile) {
      io.stdout.write(plan({ workspace: o.workspace, scratch: o.scratch, cmd: o.cmd, cwd: o.cwd, readPaths: o.readPaths }).profile);
      return 0;
    }
    const r = await runUntrusted({ ...o, forward: io });
    if (r.timedOut) {
      io.stderr.write(`run-untrusted: timed out after ${o.timeoutMs} ms\n`);
      return EXIT.TIMEOUT;
    }
    if (r.signal) return 128 + (os.constants.signals[r.signal] ?? 0);
    return r.exitCode ?? EXIT.INTERNAL;
  } catch (e) {
    io.stderr.write(`run-untrusted: ${e.message}\n`);
    if (e instanceof SandboxError && e.code === 'USAGE') io.stderr.write(`${USAGE_TEXT}\n`);
    return e instanceof SandboxError ? e.exitCode : EXIT.INTERNAL;
  }
}

const entry = process.argv[1] ? safeRealpath(process.argv[1]) : null;
if (entry !== null && entry === fs.realpathSync(fileURLToPath(import.meta.url))) {
  main(process.argv.slice(2)).then((code) => {
    process.exitCode = code;
  });
}
