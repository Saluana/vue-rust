// Node oracle: scenario contract, trace-format.md steps 1-5.
// executeScenario runs one scenario in the current process. runScenarioLine/runScenario spawn a fresh child
// process per scenario (node tooling/oracle/run.mjs <file>), so scenarios cannot share state.
import {spawn} from 'node:child_process';
import path from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {createRecorder} from './trace.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
export const casesRoot = path.resolve(here, '../../tests/language/cases');
const cli = path.join(here, 'run.mjs');

let importCount = 0;

// One macrotask boundary drains the whole microtask queue. Node emits unhandledRejection after the microtask
// queue drains, so a second boundary makes those reports visible before the list is read.
const turn = () => new Promise((resolve) => setImmediate(resolve));
const drain = async () => {
  await turn();
  await turn();
};

// Negative, non-numeric and NaN delays are treated as 0 (the fake clock has no real-time meaning).
const delayOf = (ms) => {
  const d = Number(ms);
  return Number.isFinite(d) && d > 0 ? d : 0;
};

// Deterministic fake clock. Timers fire in (due time, creation order). The job queue is drained before the
// first timer fires and again after each callback, so timers only fire at macrotask boundaries.
function makeTimers() {
  let clock = 0;
  let nextId = 1;
  const pending = new Map(); // id -> {id, due, fn}; ids increase with creation order
  function earliestDue(limit) {
    let best = null;
    for (const t of pending.values()) {
      if (t.due > limit) continue;
      if (best === null || t.due < best.due || (t.due === best.due && t.id < best.id)) best = t;
    }
    return best;
  }
  return {
    setTimeout(fn, ms) {
      if (typeof fn !== 'function') throw new TypeError('setTimeout callback must be a function');
      const id = nextId++;
      pending.set(id, {id, due: clock + delayOf(ms), fn});
      return id;
    },
    clearTimeout(id) {
      pending.delete(id);
    },
    now() {
      return clock;
    },
    async advance(ms) {
      const target = clock + delayOf(ms);
      await drain();
      for (let t = earliestDue(target); t !== null; t = earliestDue(target)) {
        pending.delete(t.id);
        clock = t.due;
        t.fn(); // a throw here propagates to the caller of advance
        await drain();
      }
      clock = target;
      await drain();
    },
  };
}

// Document "scenario" field: path relative to tests/language/cases without ".js" (basename outside that root).
export function scenarioName(file) {
  const abs = path.resolve(file);
  const rel = path.relative(casesRoot, abs);
  const inside = rel !== '' && !rel.startsWith('..') && !path.isAbsolute(rel);
  return (inside ? rel : path.basename(abs)).split(path.sep).join('/').replace(/\.js$/, '');
}

// Runs one scenario in this process and returns the trace document (not yet serialized).
// It installs process-wide state (globals, unhandledRejection listeners), so only run.mjs calls it; tests and
// goldens go through runScenario, which isolates each scenario in a child process.
export async function executeScenario(file) {
  const abs = path.resolve(file);
  const rec = createRecorder();

  // Step 1: fresh globals and fresh ordinals (the recorder is new).
  globalThis.trace = rec.trace;
  globalThis.testHost = {timers: makeTimers(), tick: drain};

  // Unhandled-rejection bookkeeping: a rejection counts while no handler is attached by the final drain.
  const open = [];
  const onRejected = (reason, promise) => {
    open.push({promise, reason});
  };
  const onHandled = (promise) => {
    const i = open.findIndex((r) => r.promise === promise);
    if (i >= 0) open.splice(i, 1);
  };
  process.on('unhandledRejection', onRejected);
  process.on('rejectionHandled', onHandled);
  try {
    let result = 'ok';
    let error = null;
    let ns = null;

    // Step 2: evaluate the module. A unique query string bypasses the module cache.
    importCount += 1;
    try {
      ns = await import(`${pathToFileURL(abs).href}?run=${importCount}`);
    } catch (e) {
      result = 'throw';
      error = rec.encode(e);
    }

    // Step 3: drain jobs.
    await drain();

    // Step 4: call run() if the module evaluated and exports it. Sync throws and rejections are both "throw".
    if (ns !== null && 'run' in ns) {
      if (typeof ns.run !== 'function') throw new TypeError(`${abs}: export "run" must be a function`);
      let threw = false;
      let reason;
      let p;
      try {
        p = ns.run();
      } catch (e) {
        threw = true;
        reason = e;
      }
      if (!threw) {
        try {
          await p;
        } catch (e) {
          threw = true;
          reason = e;
        }
      }
      if (threw) {
        result = 'throw';
        error = rec.encode(reason);
      }
      await drain();
    }

    // Step 5: final drain, then report rejections that still have no handler (in rejection order).
    await drain();
    const unhandledRejections = rec.encodeAll(open.map((r) => r.reason));
    return {scenario: scenarioName(abs), result, error, entries: rec.entries, unhandledRejections};
  } finally {
    process.off('unhandledRejection', onRejected);
    process.off('rejectionHandled', onHandled);
  }
}

// Spawns a fresh child for one scenario and returns its stdout: exactly one JSON line, newline-terminated.
export function runScenarioLine(file) {
  return new Promise((resolve, reject) => {
    const env = {...process.env};
    delete env.NODE_TEST_CONTEXT; // a child of node --test must behave like a plain node process
    const child = spawn(process.execPath, [cli, path.resolve(file)], {env, stdio: ['ignore', 'pipe', 'pipe']});
    let out = '';
    let err = '';
    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    child.stdout.on('data', (d) => {
      out += d;
    });
    child.stderr.on('data', (d) => {
      err += d;
    });
    // Real-time guard for the harness only (scenarios never see real time).
    const guard = setTimeout(() => child.kill('SIGKILL'), 60000);
    child.on('error', (e) => {
      clearTimeout(guard);
      reject(e);
    });
    child.on('close', (code) => {
      clearTimeout(guard);
      if (code !== 0) reject(new Error(`${file}: exit ${code}: ${err.trim()}`));
      else if (out.indexOf('\n') !== out.length - 1) reject(new Error(`${file}: output is not exactly one line`));
      else resolve(out);
    });
  });
}

// Spawns a fresh child for one scenario and returns the parsed trace document.
export async function runScenario(file) {
  return JSON.parse(await runScenarioLine(file));
}
