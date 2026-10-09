// Headless Chrome over the DevTools protocol (WP-X1). Zero npm dependencies: Node 26 global WebSocket.
// Flags and their reasons: tooling/chrome/README.md. Every Chrome started here is killed by close(), on process
// exit, and on SIGINT/SIGTERM/SIGHUP. Only processes owned by one launch are ever signalled (see ownedPids).
import { execFile, execFileSync, spawn } from 'node:child_process';
import fs from 'node:fs';
import fsp from 'node:fs/promises';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export const CHROME_CANDIDATES = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/google-chrome-stable',
  '/usr/bin/chromium',
];

// Fixed flags from WP-X1. README.md gives the reason for each.
export const CHROME_FLAGS = Object.freeze([
  '--remote-debugging-port=0',
  '--no-first-run',
  '--no-default-browser-check',
  '--disable-extensions',
  '--disable-gpu',
  '--disable-background-networking',
  '--disable-sync',
  '--metrics-recording-only',
  '--mute-audio',
  '--hide-scrollbars',
]);

const MANAGED_ARG = /^--(user-data-dir|remote-debugging-port|headless)(=|$)/;
const ERROR_CLASSES = { Error, TypeError, RangeError, SyntaxError, ReferenceError, EvalError, URIError };

// CHROME_PATH wins if set (and must be executable, otherwise this throws rather than silently using another browser).
export function findChrome() {
  const fromEnv = process.env.CHROME_PATH;
  if (fromEnv) {
    if (!isExecutable(fromEnv)) throw new Error(`CHROME_PATH=${fromEnv} is not an executable file`);
    return fromEnv;
  }
  return CHROME_CANDIDATES.find(isExecutable) ?? null;
}

// null when Chrome exists; otherwise the reason tests print when they skip.
export function chromeSkipReason() {
  return findChrome() ? null : 'Google Chrome not found; install it or set CHROME_PATH';
}

function isExecutable(file) {
  try {
    fs.accessSync(file, fs.constants.X_OK);
    return fs.statSync(file).isFile();
  } catch {
    return false;
  }
}

// ---- process bookkeeping -------------------------------------------------------------------------------------

const PS_ARGS = ['-A', '-o', 'pid=,ppid=,stat=,command='];

function parsePs(text) {
  const rows = [];
  for (const line of text.split('\n')) {
    const m = /^\s*(\d+)\s+(\d+)\s+(\S+)\s+(.*)$/.exec(line);
    if (m) rows.push({ pid: Number(m[1]), ppid: Number(m[2]), stat: m[3], command: m[4] });
  }
  return rows;
}

const psAsync = async () => parsePs((await execFileAsync('ps', PS_ARGS, { maxBuffer: 1 << 26 })).stdout);
const psSync = () => parsePs(execFileSync('ps', PS_ARGS, { encoding: 'utf8', maxBuffer: 1 << 26 }));

// True when `command` contains `--user-data-dir=<dir>` as a whole argument (so /a/p does not match /a/p2).
function carriesDir(command, dir) {
  const flag = `--user-data-dir=${dir}`;
  for (let i = command.indexOf(flag); i !== -1; i = command.indexOf(flag, i + 1)) {
    const end = i + flag.length;
    if (end === command.length || command[end] === ' ') return true;
  }
  return false;
}

// Processes owned by one launch: the browser's descendants (only while the browser is still our unreaped child),
// plus every process whose command line carries this launch's unique --user-data-dir. Renderer, GPU and utility
// helpers carry that flag (checked on macOS arm64, Chrome 155), which also catches helpers re-parented to launchd.
function ownedPids(rows, launch) {
  const owned = new Set();
  if (launch.pid === undefined) return owned;
  if (launch.child.exitCode === null && launch.child.signalCode === null) {
    const kids = new Map();
    for (const r of rows) {
      if (!kids.has(r.ppid)) kids.set(r.ppid, []);
      kids.get(r.ppid).push(r.pid);
    }
    const stack = [launch.pid];
    while (stack.length) {
      for (const c of kids.get(stack.pop()) ?? []) {
        if (!owned.has(c)) {
          owned.add(c);
          stack.push(c);
        }
      }
    }
    owned.add(launch.pid);
  }
  for (const r of rows) if (carriesDir(r.command, launch.userDataDir)) owned.add(r.pid);
  owned.delete(process.pid);
  return owned;
}

function tryKill(pid, sig) {
  try {
    process.kill(pid, sig);
  } catch {
    // already gone
  }
}

function signalOwned(launch, pids, sig) {
  if (launch.pid === undefined) return;
  // The browser is spawned detached, so -pid is its own process group; this reaches helpers that stayed in it.
  if (launch.child.exitCode === null && launch.child.signalCode === null) tryKill(-launch.pid, sig);
  for (const pid of pids) tryKill(pid, sig);
}

async function waitGone(launch, ms) {
  const deadline = Date.now() + ms;
  for (;;) {
    const rows = await psAsync();
    const up = new Set(rows.filter((r) => !r.stat.startsWith('Z')).map((r) => r.pid));
    const left = [...ownedPids(rows, launch)].filter((pid) => up.has(pid));
    const reaped = launch.child.exitCode !== null || launch.child.signalCode !== null;
    if (left.length === 0 && reaped) return true;
    if (Date.now() >= deadline) return false;
    await sleep(50);
  }
}

async function terminate(launch, graceMs) {
  signalOwned(launch, ownedPids(await psAsync(), launch), 'SIGTERM');
  if (await waitGone(launch, graceMs)) return;
  signalOwned(launch, ownedPids(await psAsync(), launch), 'SIGKILL');
  if (!(await waitGone(launch, 3000))) throw new Error(`Chrome (pid ${launch.pid}) survived SIGKILL`);
}

// Live launches. The exit/signal hooks below run synchronously because the process is going away.
const live = new Set();
const SIGNALS = ['SIGINT', 'SIGTERM', 'SIGHUP'];
const onSignal = (sig) => {
  for (const launch of [...live]) killNowSync(launch);
  process.exit(128 + os.constants.signals[sig]);
};

function track(launch) {
  if (live.size === 0) for (const s of SIGNALS) process.on(s, onSignal);
  live.add(launch);
}

function untrack(launch) {
  if (!live.delete(launch)) return;
  if (live.size === 0) for (const s of SIGNALS) process.off(s, onSignal);
}

const sleepCell = new Int32Array(new SharedArrayBuffer(4));

// Runs from the exit/signal hooks, so it is synchronous. It waits (up to 2 s) for the SIGKILLed processes to be gone
// before the profile is removed; removing while they are still writing can leave a half-deleted directory.
function killNowSync(launch) {
  try {
    signalOwned(launch, ownedPids(psSync(), launch), 'SIGKILL');
    const until = Date.now() + 2000;
    for (;;) {
      const rows = psSync();
      const up = new Set(rows.filter((r) => !r.stat.startsWith('Z')).map((r) => r.pid));
      if (![...ownedPids(rows, launch)].some((pid) => up.has(pid)) || Date.now() > until) break;
      Atomics.wait(sleepCell, 0, 0, 20);
    }
  } catch {
    // ps unavailable: the temp dir is still removed below
  }
  if (launch.ownsDir) {
    try {
      fs.rmSync(launch.userDataDir, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    } catch {
      // best effort on the way out
    }
  }
  launch.gone = true;
  untrack(launch);
}

process.on('exit', () => {
  for (const launch of [...live]) killNowSync(launch);
});

// ---- launch --------------------------------------------------------------------------------------------------

// Resolves with the "DevTools listening on ws://..." URL printed by Chrome on stderr, or rejects on timeout/exit.
function devToolsUrl(child, timeout) {
  return new Promise((resolve, reject) => {
    let tail = '';
    let timer;
    const finish = (err, url) => {
      clearTimeout(timer);
      child.stderr.off('data', onData);
      child.off('exit', onExit);
      child.off('error', onError);
      child.stderr.on('data', () => {}); // keep draining so Chrome never blocks on a full pipe
      if (err) reject(err);
      else resolve(url);
    };
    const onData = (chunk) => {
      tail = (tail + chunk).slice(-16384);
      const m = /DevTools listening on (ws:\/\/\S+)/.exec(tail);
      if (m) finish(null, m[1]);
    };
    const onExit = (code, signal) =>
      finish(new Error(`Chrome exited before DevTools was ready (code ${code}, signal ${signal}); stderr: ${tail.trim().slice(-600)}`));
    const onError = (err) => finish(new Error(`could not start Chrome: ${err.message}`));
    timer = setTimeout(
      () => finish(new Error(`no "DevTools listening on" line within ${timeout} ms; stderr: ${tail.trim().slice(-600)}`)),
      timeout,
    );
    child.stderr.setEncoding('utf8');
    child.stderr.on('data', onData);
    child.once('exit', onExit);
    child.once('error', onError);
  });
}

function closeLaunch(launch) {
  if (!launch.closing) {
    launch.closing = (async () => {
      try {
        if (launch.pid !== undefined) await terminate(launch, 3000);
      } finally {
        if (launch.ownsDir) await fsp.rm(launch.userDataDir, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
        launch.gone = true;
        untrack(launch);
      }
    })();
  }
  return launch.closing;
}

// Starts Chrome with a fresh temp profile (or the given userDataDir, which is then never deleted).
// Resolves with { wsUrl, port, pid, userDataDir, close() }; close() is idempotent.
export async function launchChrome(options = {}) {
  const { headless = true, userDataDir, args = [], timeout = 15000 } = options;
  if (!(Number.isFinite(timeout) && timeout > 0)) throw new TypeError('timeout must be a positive number of ms');
  for (const a of args) {
    if (typeof a !== 'string') throw new TypeError('launchChrome: args must be strings');
    if (MANAGED_ARG.test(a)) throw new Error(`launchChrome: ${a} is set by the runner; use the userDataDir or headless option`);
  }
  const executable = findChrome();
  if (!executable) throw new Error('Google Chrome not found; install it or set CHROME_PATH');
  const ownsDir = userDataDir === undefined;
  const dir = ownsDir ? await fsp.mkdtemp(path.join(os.tmpdir(), 'vn-chrome-')) : path.resolve(userDataDir);
  const flags = [...(headless ? ['--headless=new'] : []), ...CHROME_FLAGS, `--user-data-dir=${dir}`, ...args];
  const child = spawn(executable, flags, { stdio: ['ignore', 'ignore', 'pipe'], detached: true });
  const launch = { child, pid: child.pid, userDataDir: dir, ownsDir, closing: null, gone: false };
  track(launch);
  try {
    const wsUrl = await devToolsUrl(child, timeout);
    return {
      wsUrl,
      port: Number(new URL(wsUrl).port),
      pid: child.pid,
      userDataDir: dir,
      close: () => closeLaunch(launch),
    };
  } catch (err) {
    await closeLaunch(launch).catch((closeErr) => attachCleanupError(err, closeErr)); // the launch error is reported
    throw err;
  }
}

// ---- DevTools client -----------------------------------------------------------------------------------------

function withTimeout(promise, ms, what) {
  let timer;
  const limit = new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Error(`${what} timed out after ${ms} ms`)), ms);
  });
  return Promise.race([promise, limit]).finally(() => clearTimeout(timer));
}

// Text for one console argument (RemoteObject). Objects are described, not serialised.
function remoteText(a) {
  if (a && 'value' in a) return typeof a.value === 'string' ? a.value : JSON.stringify(a.value);
  if (a && a.unserializableValue !== undefined) return a.unserializableValue;
  return a?.description ?? a?.type ?? '';
}

function exceptionMessage(details = {}) {
  const desc = details.exception?.description ?? details.exception?.value ?? details.text ?? '';
  return String(desc).split('\n')[0];
}

export class CdpSession {
  static connect(url, { timeout = 10000 } = {}) {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(url);
      const timer = setTimeout(() => {
        ws.close();
        reject(new Error(`CDP connect to ${url} timed out after ${timeout} ms`));
      }, timeout);
      ws.addEventListener('open', () => {
        clearTimeout(timer);
        resolve(new CdpSession(ws));
      }, { once: true });
      ws.addEventListener('error', () => {
        clearTimeout(timer);
        reject(new Error(`CDP connect to ${url} failed`));
      }, { once: true });
    });
  }

  constructor(ws) {
    this.ws = ws;
    this.closed = false;
    this.nextId = 1;
    this.pending = new Map();   // id -> { method, resolve, reject }
    this.listeners = new Map(); // method -> Set of fn(params, sessionId)
    this.waiters = new Set();   // { method, predicate, resolve, reject, timer }
    this.console = [];          // { sessionId, type, text } from Runtime.consoleAPICalled
    this.exceptions = [];       // { sessionId, message } from Runtime.exceptionThrown
    ws.addEventListener('message', (ev) => this.#receive(ev.data));
    ws.addEventListener('close', () => this.#shutdown('CDP connection closed'));
  }

  // sessionId is the flattened target session (omit for browser-level commands).
  send(method, params = {}, sessionId) {
    if (this.closed) return Promise.reject(new Error(`CDP connection closed; cannot send ${method}`));
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { method, resolve, reject });
      const msg = { id, method, params };
      if (sessionId !== undefined) msg.sessionId = sessionId;
      try {
        this.ws.send(JSON.stringify(msg));
      } catch (err) {
        this.pending.delete(id);
        reject(err);
      }
    });
  }

  // Calls fn(params, sessionId) for every event named `method`. Returns an unsubscribe function.
  on(method, fn) {
    if (!this.listeners.has(method)) this.listeners.set(method, new Set());
    this.listeners.get(method).add(fn);
    return () => this.listeners.get(method)?.delete(fn);
  }

  // Resolves with the params of the first `method` event for which predicate(params, sessionId) is true.
  waitForEvent(method, predicate = () => true, timeout = 10000) {
    return new Promise((resolve, reject) => {
      if (this.closed) return reject(new Error(`CDP connection closed while waiting for ${method}`));
      const w = { method, predicate, resolve, reject, timer: null };
      w.timer = setTimeout(() => {
        this.waiters.delete(w);
        reject(new Error(`timed out after ${timeout} ms waiting for ${method}`));
      }, timeout);
      this.waiters.add(w);
    });
  }

  async close() {
    if (this.closed) return;
    const gone = this.ws.readyState === WebSocket.CLOSED
      ? Promise.resolve()
      : new Promise((resolve) => this.ws.addEventListener('close', resolve, { once: true }));
    try {
      this.ws.close();
    } catch {
      // already closing
    }
    await withTimeout(gone, 1000, 'CDP close').catch(() => {});
    this.#shutdown('CDP connection closed');
  }

  #receive(data) {
    let msg;
    try {
      msg = JSON.parse(String(data));
    } catch {
      return;
    }
    if (msg.id !== undefined) {
      const p = this.pending.get(msg.id);
      if (!p) return;
      this.pending.delete(msg.id);
      if (msg.error) p.reject(new Error(`${p.method} failed: ${msg.error.message}`));
      else p.resolve(msg.result ?? {});
      return;
    }
    const { method, params = {}, sessionId } = msg;
    if (method === 'Runtime.consoleAPICalled') {
      this.console.push({ sessionId, type: params.type, text: (params.args ?? []).map(remoteText).join(' ') });
    } else if (method === 'Runtime.exceptionThrown') {
      this.exceptions.push({ sessionId, message: exceptionMessage(params.exceptionDetails) });
    }
    for (const fn of [...(this.listeners.get(method) ?? [])]) {
      try {
        fn(params, sessionId);
      } catch (err) {
        queueMicrotask(() => { throw err; }); // a broken listener must fail loudly, not stall the socket
      }
    }
    for (const w of [...this.waiters]) {
      let hit = false;
      try {
        hit = w.method === method && w.predicate(params, sessionId);
      } catch (err) {
        this.waiters.delete(w);
        clearTimeout(w.timer);
        w.reject(err);
        continue;
      }
      if (hit) {
        this.waiters.delete(w);
        clearTimeout(w.timer);
        w.resolve(params);
      }
    }
  }

  #shutdown(reason) {
    if (this.closed) return;
    this.closed = true;
    for (const p of this.pending.values()) p.reject(new Error(`${reason} (pending ${p.method})`));
    this.pending.clear();
    for (const w of this.waiters) {
      clearTimeout(w.timer);
      w.reject(new Error(`${reason} while waiting for ${w.method}`));
    }
    this.waiters.clear();
  }
}

// ---- values crossing into and out of the page ----------------------------------------------------------------
// Results: the page JSON-encodes its value with vnEncode; Node decodes with vnDecode. Non-JSON values get tags:
//   undefined -> {"$t":"undefined"} (decoded back to undefined, also inside arrays and objects)
//   NaN, Infinity, -Infinity, -0 -> the strings 'NaN', 'Infinity', '-Infinity', '-0'
//   BigInt -> its decimal string; functions, symbols, non-plain objects (Date, Map, DOM nodes...) -> TypeError
//   cyclic values -> TypeError; a plain object key '$t' is reserved -> TypeError
// Arguments: encodeArg (Node) tags NaN/Infinity/-Infinity/-0/BigInt/undefined so the page receives real values.

// Page-side encoder. Its source is embedded into the evaluated expression, so it must stay self-contained.
function vnEncode(v, stack) {
  if (v === undefined) return { $t: 'undefined' };
  if (v === null || typeof v === 'string' || typeof v === 'boolean') return v;
  if (typeof v === 'number') {
    if (v !== v) return 'NaN';
    if (v === Infinity) return 'Infinity';
    if (v === -Infinity) return '-Infinity';
    if (v === 0 && 1 / v < 0) return '-0';
    return v;
  }
  if (typeof v === 'bigint') return v.toString();
  if (typeof v !== 'object') throw new TypeError('evaluate: cannot return a ' + typeof v + ' value');
  if (stack.indexOf(v) !== -1) throw new TypeError('evaluate: cannot return a cyclic value');
  const proto = Object.getPrototypeOf(v);
  if (!Array.isArray(v) && proto !== Object.prototype && proto !== null) {
    const name = (v.constructor && v.constructor.name) || 'unknown';
    throw new TypeError('evaluate: cannot return a non-plain object (' + name + '); copy it into a plain object first');
  }
  stack.push(v);
  try {
    if (Array.isArray(v)) {
      const out = [];
      for (let i = 0; i < v.length; i++) out.push(vnEncode(v[i], stack));
      return out;
    }
    const out = {};
    for (const k of Object.keys(v)) {
      if (k === '$t') throw new TypeError("evaluate: the key '$t' is reserved");
      Object.defineProperty(out, k, { value: vnEncode(v[k], stack), enumerable: true, writable: true, configurable: true });
    }
    return out;
  } finally {
    stack.pop();
  }
}

// Decoder used by Node for results and by the page for arguments. Also self-contained (embedded as source).
function vnDecode(x) {
  if (x === null || typeof x !== 'object') return x;
  if (Array.isArray(x)) {
    const out = [];
    for (let i = 0; i < x.length; i++) out.push(vnDecode(x[i]));
    return out;
  }
  if (Object.prototype.hasOwnProperty.call(x, '$t')) {
    switch (x.$t) {
      case 'undefined': return undefined;
      case 'NaN': return NaN;
      case 'Infinity': return Infinity;
      case '-Infinity': return -Infinity;
      case '-0': return -0;
      case 'bigint': return BigInt(x.v);
      default: throw new TypeError('unknown value tag ' + x.$t);
    }
  }
  const out = {};
  for (const k of Object.keys(x)) {
    Object.defineProperty(out, k, { value: vnDecode(x[k]), enumerable: true, writable: true, configurable: true });
  }
  return out;
}

function encodeArg(v, stack = []) {
  if (v === undefined) return { $t: 'undefined' };
  if (v === null || typeof v === 'string' || typeof v === 'boolean') return v;
  if (typeof v === 'number') {
    if (v !== v) return { $t: 'NaN' };
    if (v === Infinity) return { $t: 'Infinity' };
    if (v === -Infinity) return { $t: '-Infinity' };
    if (v === 0 && 1 / v < 0) return { $t: '-0' };
    return v;
  }
  if (typeof v === 'bigint') return { $t: 'bigint', v: v.toString() };
  if (typeof v !== 'object') throw new TypeError(`evaluate: cannot pass a ${typeof v} argument to the page`);
  if (stack.includes(v)) throw new TypeError('evaluate: cannot pass a cyclic argument to the page');
  const proto = Object.getPrototypeOf(v);
  if (!Array.isArray(v) && proto !== Object.prototype && proto !== null) {
    throw new TypeError('evaluate: arguments must be plain data (objects, arrays, primitives)');
  }
  stack.push(v);
  try {
    if (Array.isArray(v)) {
      const out = [];
      for (let i = 0; i < v.length; i++) out.push(encodeArg(v[i], stack));
      return out;
    }
    const out = {};
    for (const k of Object.keys(v)) {
      if (k === '$t') throw new TypeError("evaluate: the key '$t' is reserved");
      Object.defineProperty(out, k, { value: encodeArg(v[k], stack), enumerable: true, writable: true, configurable: true });
    }
    return out;
  } finally {
    stack.pop();
  }
}

// Builds the expression sent to Runtime.evaluate. The wrapper never throws; it returns a JSON envelope so the
// original page error name and message come back unchanged.
function buildExpression(target, args) {
  let call;
  if (typeof target === 'string') {
    if (args.length) throw new TypeError('evaluate: extra arguments are only supported when evaluating a function');
    call = `(0, eval)(${JSON.stringify(target)})`;
  } else if (typeof target === 'function') {
    const src = Function.prototype.toString.call(target);
    if (/\{\s*\[native code\]\s*\}$/.test(src)) {
      throw new TypeError('evaluate: native functions cannot be sent to the page; wrap them in an arrow function');
    }
    const payload = JSON.stringify(JSON.stringify(args.map((a) => encodeArg(a))));
    call = `(${src}).apply(undefined, vnDecode(JSON.parse(${payload})))`;
  } else {
    throw new TypeError('evaluate expects a function or an expression string');
  }
  return `(async () => {
${vnDecode.toString()}
${vnEncode.toString()}
try {
  const r = await ${call};
  return JSON.stringify({ ok: true, v: vnEncode(r, []) });
} catch (e) {
  return JSON.stringify({
    ok: false,
    name: String((e && e.name) || 'Error'),
    message: String(e && e.message !== undefined ? e.message : e),
    stack: String((e && e.stack) || ''),
  });
}
})()`;
}

function pageError(env) {
  const Cls = Object.hasOwn(ERROR_CLASSES, env.name) ? ERROR_CLASSES[env.name] : Error;
  const err = new Cls(env.message);
  if (env.name && err.name !== env.name) err.name = env.name;
  err.pageStack = env.stack;
  return err;
}

// Syntax errors in the wrapper itself (e.g. an unparsable function source) arrive as exceptionDetails.
function compileError(details) {
  const text = String(details.exception?.description ?? details.text ?? 'evaluate failed');
  const first = text.split('\n')[0];
  const m = /^(\w+): (.*)$/.exec(first);
  const Cls = m && Object.hasOwn(ERROR_CLASSES, m[1]) ? ERROR_CLASSES[m[1]] : Error;
  return new Cls(m ? m[2] : first);
}

function toUrl(target) {
  if (typeof target !== 'string') throw new TypeError('goto(urlOrFile) expects a string');
  if (/^(https?|file|about|data):/i.test(target)) return target;
  const abs = path.resolve(target);
  if (!fs.existsSync(abs)) throw new Error(`goto: no such file ${abs}`);
  return pathToFileURL(abs).href;
}

// ---- pages ---------------------------------------------------------------------------------------------------

export class Page {
  #timeout;
  #closing = null;

  constructor({ cdp, sessionId, browser, timeout }) {
    this.cdp = cdp;
    this.sessionId = sessionId;
    this.browser = browser;
    this.#timeout = timeout;
  }

  get timeout() {
    return this.#timeout;
  }

  set timeout(ms) {
    if (!(Number.isFinite(ms) && ms > 0)) throw new TypeError('timeout must be a positive number of ms');
    this.#timeout = ms;
  }

  get pid() {
    return this.browser.pid;
  }

  get userDataDir() {
    return this.browser.userDataDir;
  }

  get wsUrl() {
    return this.browser.wsUrl;
  }

  // Console calls made by this page, in order: { type: 'log' | 'error' | 'warning' | ..., text }.
  get consoleLog() {
    return this.cdp.console.filter((e) => e.sessionId === this.sessionId).map(({ type, text }) => ({ type, text }));
  }

  // Uncaught page exceptions: { message } (first line of the error description).
  get exceptions() {
    return this.cdp.exceptions.filter((e) => e.sessionId === this.sessionId).map(({ message }) => ({ message }));
  }

  // Raw CDP command on this page's session.
  send(method, params = {}) {
    return withTimeout(this.cdp.send(method, params, this.sessionId), this.#timeout, method);
  }

  async goto(urlOrFile) {
    const url = toUrl(urlOrFile);
    const loaded = this.cdp.waitForEvent('Page.loadEventFired', (_p, sid) => sid === this.sessionId, this.#timeout);
    loaded.catch(() => {}); // surfaced by the await below; this only prevents an unhandled rejection
    const res = await this.send('Page.navigate', { url });
    if (res.errorText) throw new Error(`navigation to ${url} failed: ${res.errorText}`);
    await loaded;
  }

  // Loads an HTML string verbatim (no implicit doctype). The page origin is about:blank: relative URLs do not resolve.
  async setContent(html) {
    if (typeof html !== 'string') throw new TypeError('setContent(html) expects a string');
    const { frameTree } = await this.send('Page.getFrameTree');
    await this.send('Page.setDocumentContent', { frameId: frameTree.frame.id, html });
    await this.evaluate(async () => {
      if (document.readyState !== 'complete') await new Promise((done) => addEventListener('load', done, { once: true }));
    });
  }

  // target: a function (its source is sent, so it must be self-contained) or an expression string.
  async evaluate(target, ...args) {
    const expression = buildExpression(target, args);
    const res = await withTimeout(
      this.cdp.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true }, this.sessionId),
      this.#timeout,
      'evaluate',
    );
    if (res.exceptionDetails) throw compileError(res.exceptionDetails);
    if (res.result?.type !== 'string') throw new Error('evaluate: unexpected CDP result');
    const env = JSON.parse(res.result.value);
    if (!env.ok) throw pageError(env);
    return vnDecode(env.v);
  }

  async viewport(width, height, dpr = 1) {
    if (!Number.isInteger(width) || width <= 0) throw new TypeError('viewport width must be a positive integer');
    if (!Number.isInteger(height) || height <= 0) throw new TypeError('viewport height must be a positive integer');
    if (!(typeof dpr === 'number' && dpr > 0)) throw new TypeError('viewport dpr must be a positive number');
    await this.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: dpr, mobile: false });
  }

  // Writes a PNG of the current viewport and returns its absolute path.
  async screenshot(file) {
    const target = path.resolve(file);
    const { data } = await this.send('Page.captureScreenshot', { format: 'png' });
    await fsp.mkdir(path.dirname(target), { recursive: true });
    await fsp.writeFile(target, Buffer.from(data, 'base64'));
    return target;
  }

  // Closes the DevTools connection and the browser (and deletes its temp profile). Idempotent.
  close() {
    if (!this.#closing) {
      this.#closing = (async () => {
        await this.cdp.close();
        await this.browser.close();
      })();
    }
    return this.#closing;
  }
}

// Keeps the primary error as the one thrown, but records a failed cleanup on it instead of discarding it.
function attachCleanupError(err, closeErr) {
  if (err && typeof err === 'object') err.cleanupError = closeErr;
}

// Opens a page on a fresh browser. The caller must call page.close(). Default viewport 800x600 at DPR 1.
export async function openPage(options = {}) {
  const { viewport = {}, timeout = 15000, ...launch } = options;
  if (!(Number.isFinite(timeout) && timeout > 0)) throw new TypeError('timeout must be a positive number of ms');
  const { width = 800, height = 600, dpr = 1 } = viewport;
  const browser = await launchChrome({ ...launch, timeout });
  let cdp = null;
  try {
    cdp = await CdpSession.connect(browser.wsUrl, { timeout });
    const { targetId } = await withTimeout(cdp.send('Target.createTarget', { url: 'about:blank' }), timeout, 'Target.createTarget');
    const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
    await cdp.send('Page.enable', {}, sessionId);
    await cdp.send('Runtime.enable', {}, sessionId);
    const page = new Page({ cdp, sessionId, browser, timeout });
    await page.viewport(width, height, dpr);
    return page;
  } catch (err) {
    if (cdp) await cdp.close();
    await browser.close().catch((closeErr) => attachCleanupError(err, closeErr));
    throw err;
  }
}

// Runs fn(page) on a fresh page and always closes the browser afterwards, also when fn throws.
export async function withPage(options, fn) {
  if (typeof options === 'function') {
    fn = options;
    options = {};
  }
  if (typeof fn !== 'function') throw new TypeError('withPage(options, fn) needs a function');
  const page = await openPage(options);
  let result;
  try {
    result = await fn(page);
  } catch (err) {
    await page.close().catch((closeErr) => attachCleanupError(err, closeErr)); // the callback's error is reported
    throw err;
  }
  await page.close();
  return result;
}

// ---- static file server --------------------------------------------------------------------------------------

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.htm': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.map': 'application/json; charset=utf-8',
  '.wasm': 'application/wasm',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.txt': 'text/plain; charset=utf-8',
  '.woff2': 'font/woff2',
};

// Serves `dir` on 127.0.0.1 at an ephemeral port. Resolves with { url, port, close() }.
// headers: extra response headers, e.g. Cross-Origin-Opener-Policy / Cross-Origin-Embedder-Policy.
export async function serveDirectory(dir, { headers = {} } = {}) {
  const root = path.resolve(dir);
  if (!fs.statSync(root).isDirectory()) throw new Error(`serveDirectory: ${root} is not a directory`);
  const server = http.createServer((req, res) => {
    if (req.method !== 'GET' && req.method !== 'HEAD') {
      res.writeHead(405, { Allow: 'GET, HEAD' }).end();
      return;
    }
    let rel;
    try {
      rel = decodeURIComponent(new URL(req.url, 'http://localhost').pathname);
    } catch {
      res.writeHead(400).end();
      return;
    }
    if (rel.endsWith('/')) rel += 'index.html';
    const file = path.join(root, rel);
    if (file !== root && !file.startsWith(root + path.sep)) {
      res.writeHead(403).end();
      return;
    }
    fs.stat(file, (err, st) => {
      if (err || !st.isFile()) {
        res.writeHead(404).end();
        return;
      }
      res.writeHead(200, {
        'Content-Type': MIME[path.extname(file).toLowerCase()] ?? 'application/octet-stream',
        'Content-Length': st.size,
        'Cache-Control': 'no-store',
        ...headers,
      });
      if (req.method === 'HEAD') {
        res.end();
        return;
      }
      const body = fs.createReadStream(file);
      body.on('error', () => res.destroy());
      body.pipe(res);
    });
  });
  return new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      resolve({
        url: `http://127.0.0.1:${port}/`,
        port,
        close: () =>
          new Promise((done, fail) => {
            server.close((e) => (e ? fail(e) : done()));
            server.closeAllConnections();
          }),
      });
    });
  });
}
