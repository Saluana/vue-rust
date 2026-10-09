// Deterministic fake clock of the UI oracle (trace-format.md, testHost.timers; ui-scenario-host.md). Browser-safe: no Node
// or DOM APIs, so the page loads this file as a module and the tests run it under Node. The algorithm is the one of
// makeTimers in tooling/oracle/node-runner.mjs: timers fire in (due time, creation order), and the job queue is drained
// before the first timer fires and after each callback. requestAnimationFrame callbacks are timers FRAME_MS apart.

export const FRAME_MS = 16;

// Negative, non-numeric and NaN delays count as 0: the clock has no real-time meaning.
export function delayOf(ms) {
  const d = Number(ms);
  return Number.isFinite(d) && d > 0 ? d : 0;
}

// drain: async () => void, the host's macrotask boundary, which also empties the job queue.
export function createClock(drain) {
  if (typeof drain !== 'function') throw new TypeError('createClock needs a drain function');
  let clock = 0;
  let nextId = 1;
  const pending = new Map(); // id -> { id, due, fn }; ids grow with creation order

  function earliestDue(limit) {
    let best = null;
    for (const t of pending.values()) {
      if (t.due > limit) continue;
      if (best === null || t.due < best.due || (t.due === best.due && t.id < best.id)) best = t;
    }
    return best;
  }

  const timers = {
    setTimeout(fn, ms = 0, ...args) {
      if (typeof fn !== 'function') throw new TypeError('setTimeout callback must be a function');
      const id = nextId++;
      pending.set(id, { id, due: clock + delayOf(ms), fn: () => fn(...args) });
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
    requestAnimationFrame(cb) {
      if (typeof cb !== 'function') throw new TypeError('requestAnimationFrame callback must be a function');
      return timers.setTimeout(() => cb(clock), FRAME_MS);
    },
  };
  return timers;
}
