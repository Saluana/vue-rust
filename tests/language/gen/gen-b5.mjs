// Generates tests/language/promise-programs/programs.json (WP-B5). Node 26 (V8) is the oracle: every program runs here with
// real Promises, and the outcome is written next to the program. The op language is documented in
// tests/language/promise-programs/README.md. Deterministic: the random programs come from a seeded generator, so two runs
// write identical bytes. `node tests/language/gen/gen-b5.mjs` rewrites the file; `runNode` is also used by tick-table.mjs.
import {writeFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import util from 'node:util';

const here = path.dirname(fileURLToPath(import.meta.url));
const outFile = path.resolve(here, '../promise-programs/programs.json');

const turn = () => new Promise((resolve) => setImmediate(resolve));
const drain = async () => {
  await turn();
  await turn();
};

const render = (v) => (typeof v === 'string' ? v : String(v));
const unquote = (s) => (s.startsWith("'") && s.endsWith("'") ? s.slice(1, -1) : s);

// Steps of an async body become a generator; the driver awaits every yielded value with a real `await`, so the only
// ticks are the ones `await` costs. Blocks are plain `yield*` calls, so they add no ticks.
function* runSteps(steps, env) {
  for (const st of steps) {
    switch (st.s) {
      case 'await':
        yield env.item(st.v);
        break;
      case 'log':
        env.log.push(st.label);
        break;
      case 'logCaught':
        env.log.push('caught:' + render(env.caught));
        break;
      case 'return':
        return {ret: env.item(st.v)};
      case 'throw':
        throw env.item(st.v);
      case 'try': {
        let result;
        try {
          result = yield* runSteps(st.body, env);
        } catch (e) {
          if (!st.catch) throw e;
          env.caught = e;
          result = yield* runSteps(st.catch, env);
        } finally {
          if (st.finally) yield* runSteps(st.finally, env);
        }
        if (result) return result;
        break;
      }
      default:
        throw new Error('unknown step ' + st.s);
    }
  }
  return undefined;
}

async function drive(gen) {
  let step = gen.next();
  for (;;) {
    if (step.done) return step.value ? step.value.ret : undefined;
    let input;
    let thrown = false;
    let reason;
    try {
      input = await step.value;
    } catch (e) {
      thrown = true;
      reason = e;
    }
    step = thrown ? gen.throw(reason) : gen.next(input);
  }
}

// The oracle: runs one program with real promises and returns its outcome.
export async function runNode(prog) {
  const log = [];
  const table = new Map();
  const resolvers = new Map();
  const order = [];
  const open = [];
  const onRejected = (reason, promise) => open.push({promise, reason});
  const onHandled = (promise) => {
    const i = open.findIndex((r) => r.promise === promise);
    if (i >= 0) open.splice(i, 1);
  };
  process.on('unhandledRejection', onRejected);
  process.on('rejectionHandled', onHandled);
  try {
    const item = (x) => {
      if (typeof x === 'string' || typeof x === 'number') return x;
      if (x.undef) return undefined;
      if (x.ref !== undefined) {
        if (!table.has(x.ref)) throw new Error(`${prog.name}: unknown ref ${x.ref}`);
        return table.get(x.ref);
      }
      throw new Error(`${prog.name}: bad item ${JSON.stringify(x)}`);
    };
    const handler = (h) => {
      switch (h.h) {
        case 'none':
          return undefined;
        case 'log':
          return () => {
            log.push(h.label);
            return undefined;
          };
        case 'logArg':
          return (v) => {
            log.push(render(v));
            return undefined;
          };
        case 'return':
          return () => item(h.v);
        case 'throw':
          return () => {
            throw item(h.v);
          };
        default:
          throw new Error(`${prog.name}: bad handler ${JSON.stringify(h)}`);
      }
    };
    const thenable = (id, mode, v) => ({
      then(res, rej) {
        log.push('then:' + id);
        if (mode === 'resolve') res(item(v));
        else if (mode === 'reject') rej(item(v));
        else if (mode === 'throw') throw item(v);
        else if (mode === 'twice') {
          res(item(v));
          res('late');
        }
      },
    });
    for (const op of prog.ops) {
      switch (op.op) {
        case 'new': {
          let res;
          let rej;
          const p = new Promise((a, b) => {
            res = a;
            rej = b;
          });
          table.set(op.id, p);
          resolvers.set(op.id, {res, rej});
          order.push(op.id);
          break;
        }
        case 'withResolvers': {
          const w = Promise.withResolvers();
          table.set(op.id, w.promise);
          resolvers.set(op.id, {res: w.resolve, rej: w.reject});
          order.push(op.id);
          break;
        }
        case 'resolve':
          resolvers.get(op.id).res(item(op.v));
          break;
        case 'reject':
          resolvers.get(op.id).rej(item(op.v));
          break;
        case 'thenable':
          table.set(op.id, thenable(op.id, op.mode, op.v));
          break;
        case 'then':
          table.set(op.id, table.get(op.src).then(handler(op.f), handler(op.r)));
          order.push(op.id);
          break;
        case 'catch':
          table.set(op.id, table.get(op.src).catch(handler(op.r)));
          order.push(op.id);
          break;
        case 'finally':
          table.set(op.id, table.get(op.src).finally(handler(op.f)));
          order.push(op.id);
          break;
        case 'static': {
          const args = op.fn === 'resolve' || op.fn === 'reject' ? [item(op.items[0])] : [op.items.map(item)];
          table.set(op.id, Promise[op.fn](...args));
          order.push(op.id);
          break;
        }
        case 'log':
          log.push(op.label);
          break;
        case 'queue': {
          const label = op.label;
          Promise.resolve().then(() => {
            log.push(label);
          });
          break;
        }
        case 'async': {
          const env = {log, item, caught: undefined};
          table.set(op.id, drive(runSteps(op.steps, env)));
          order.push(op.id);
          break;
        }
        default:
          throw new Error(`${prog.name}: unknown op ${op.op}`);
      }
    }
    await drain();
    const unhandled = open.map((r) => render(r.reason));
    // States: pending is read with inspect; settled values come from capture handlers attached after
    // the outcome above was recorded (they only add jobs; the second drain runs them).
    const promises = order.map((id) => table.get(id));
    const pending = promises.map((p) => util.inspect(p).includes("<pending>"));
    const outcome = new Map();
    promises.forEach((p, i) => {
      p.then(
        (v) => outcome.set(i, {ok: true, v}),
        (r) => outcome.set(i, {ok: false, v: r}),
      );
    });
    await drain();
    const states = order.map((id, i) => {
      if (pending[i]) return [id, "pending"];
      const o = outcome.get(i);
      return [id, (o.ok ? "fulfilled:" : "rejected:") + render(o.v)];
    });
    return {log, states, unhandled};
  } finally {
    process.off('unhandledRejection', onRejected);
    process.off('rejectionHandled', onHandled);
  }
}

// ----- constructors for the hand-designed programs -----
const N = (id) => ({op: 'new', id});
const WR = (id) => ({op: 'withResolvers', id});
const RES = (id, v) => ({op: 'resolve', id, v});
const REJ = (id, v) => ({op: 'reject', id, v});
const TH = (id, mode, v = {undef: true}) => ({op: "thenable", id, mode, v});
const THEN = (id, src, f = NONE, r = NONE) => ({op: 'then', id, src, f, r});
const CATCH = (id, src, r = NONE) => ({op: 'catch', id, src, r});
const FIN = (id, src, f = NONE) => ({op: 'finally', id, src, f});
const ST = (id, fn, items) => ({op: 'static', id, fn, items});
const LOG = (label) => ({op: 'log', label});
const Q = (label) => ({op: 'queue', label});
const ASYNC = (id, steps) => ({op: 'async', id, steps});
const NONE = {h: 'none'};
const LG = (label) => ({h: 'log', label});
const LA = {h: 'logArg'};
const RT = (v) => ({h: 'return', v});
const TW = (v) => ({h: 'throw', v});
const R = (id) => ({ref: id});
const U = {undef: true};
const AW = (v) => ({s: 'await', v});
const SL = (label) => ({s: 'log', label});
const SR = (v) => ({s: 'return', v});
const SX = (v) => ({s: 'throw', v});
const TRY = (body, c = null, f = null) => ({s: 'try', body, catch: c, finally: f});
const CAUGHT = {s: 'logCaught'};

const HAND = [];
const P = (name, doc, ops) => HAND.push({name, doc, ops: ops.flat()});

// A. then, catch and finally ordering
P('then-resolved-queues-at-once', 'then on a resolved promise queues its reaction immediately; sync log comes first.', [N('p1'), RES('p1', 'x'), THEN('t', 'p1', LA), LOG('sync')]);
P('then-pending-waits', 'then on a pending promise waits for the resolution.', [N('p1'), THEN('t', 'p1', LA), LOG('sync'), RES('p1', 'later')]);
P('chain-of-three', 'three chained thens run one after another.', [N('p1'), RES('p1', 'v'), THEN('a', 'p1', LA), THEN('b', 'a', LA), THEN('c', 'b', LA)]);
P('two-branches', 'two branches off one promise interleave by registration.', [ST('p0', 'resolve', ['s']), THEN('a', 'p0', LG('a1')), THEN('b', 'p0', LG('b1')), THEN('a2', 'a', LG('a2')), THEN('b2', 'b', LG('b2'))]);
P('catch-passes-fulfilment', 'catch on a fulfilled promise passes the value through.', [N('p1'), RES('p1', 'ok'), CATCH('c', 'p1', LG('no')), THEN('t', 'c', LA)]);
P('catch-handles-rejection', 'catch receives the reason.', [N('p1'), REJ('p1', 'bad'), CATCH('c', 'p1', LA)]);
P('catch-recovers-with-return', 'a catch that returns recovers the chain.', [N('p1'), REJ('p1', 'bad'), CATCH('c', 'p1', RT('recovered')), THEN('t', 'c', LA)]);
P('catch-rethrows', 'a throwing catch rejects the derived promise with the new reason.', [N('p1'), REJ('p1', 'one'), CATCH('c', 'p1', TW('two')), CATCH('d', 'c', LA)]);
P('then-handler-throws', 'a throwing fulfilment handler rejects the derived promise.', [N('p1'), RES('p1', 'v'), THEN('t', 'p1', TW('boom')), CATCH('c', 't', LA)]);
P('then-returns-resolved-promise', 'returning a resolved promise from a handler adopts it (two extra ticks).', [ST('q', 'resolve', ['q']), N('p1'), RES('p1', 'v'), THEN('t', 'p1', RT(R('q'))), THEN('u', 't', LA), LOG('sync')]);
P('then-returns-pending-promise', 'returning a pending promise waits for it.', [N('q'), N('p1'), RES('p1', 'v'), THEN('t', 'p1', RT(R('q'))), THEN('u', 't', LA), RES('q', 'z')]);
P('then-returns-rejected-promise', 'returning a rejected promise rejects the derived one.', [ST('q', 'reject', ['no']), N('p1'), RES('p1', 'v'), THEN('t', 'p1', RT(R('q'))), CATCH('c', 't', LA)]);
P('finally-passthrough-value', 'finally runs its callback with no argument and passes the value through.', [N('p1'), RES('p1', 'v'), FIN('f', 'p1', LG('f')), THEN('t', 'f', LA)]);
P('finally-passthrough-rejection', 'finally passes a rejection through.', [N('p1'), REJ('p1', 'e'), FIN('f', 'p1', LG('f')), CATCH('c', 'f', LA)]);
P('finally-throw-replaces', 'a throwing finally callback replaces the outcome.', [N('p1'), REJ('p1', 'orig'), FIN('f', 'p1', TW('replaced')), CATCH('c', 'f', LA)]);
P('finally-return-ignored', 'a value returned by finally is ignored.', [N('p1'), RES('p1', 'v'), FIN('f', 'p1', RT('ignored')), THEN('t', 'f', LA)]);
P('finally-returns-pending-promise', 'finally waits for a promise returned by its callback.', [N('q'), N('p1'), RES('p1', 'v'), FIN('f', 'p1', RT(R('q'))), THEN('t', 'f', LA), RES('q', 'w')]);
P('finally-nonfunction', 'finally with a non-callable argument passes through.', [N('p1'), RES('p1', 3), FIN('f', 'p1', {h: 'none'}), THEN('t', 'f', LA)]);
P('catch-without-handler-stays-unhandled', 'a catch with no handler leaves the derived rejection unhandled.', [N('p1'), REJ('p1', 'lost'), CATCH('c', 'p1')]);
P('finally-chains-ticks', 'finally adds ticks before its derived promise settles.', [N('p1'), RES('p1', 1), FIN('f', 'p1', LG('fin')), THEN('F', 'f', LG('F')), CHAIN_STEPS('d', ['a', 'b', 'c', 'd', 'e'])]);
P('then-chain-with-reject-midway', 'a rejection midway skips the fulfilment handlers.', [N('p1'), RES('p1', 1), THEN('a', 'p1', TW('mid')), THEN('b', 'a', LG('skipped')), CATCH('c', 'b', LA)]);
P('then-same-handler-twice', 'one handler registered twice runs twice, in order.', [N('p1'), RES('p1', 'x'), THEN('a', 'p1', LA), THEN('b', 'p1', LA)]);

// B. resolution with promises and thenables
P('resolve-with-resolved-promise', 'resolving with a resolved promise adopts it (two extra ticks).', [ST('q', 'resolve', ['q']), N('p1'), RES('p1', R('q')), THEN('t', 'p1', LA), LOG('sync')]);
P('resolve-with-pending-promise', 'resolving with a pending promise adopts its later value.', [N('q'), N('p1'), RES('p1', R('q')), THEN('t', 'p1', LA), RES('q', 'later')]);
P('resolve-with-rejected-promise', 'resolving with a rejected promise rejects the outer one.', [ST('q', 'reject', ['bad']), N('p1'), RES('p1', R('q')), CATCH('c', 'p1', LA)]);
P('resolve-with-sync-thenable', 'a thenable that resolves synchronously is adopted in a job.', [TH('t1', 'resolve', 'tv'), N('p1'), RES('p1', R('t1')), THEN('t', 'p1', LA), LOG('sync')]);
P('resolve-with-rejecting-thenable', 'a thenable that rejects rejects the promise.', [TH('t1', 'reject', 'no'), N('p1'), RES('p1', R('t1')), CATCH('c', 'p1', LA)]);
P('resolve-with-throwing-thenable', 'a thenable whose then throws rejects the promise with the thrown value.', [TH('t1', 'throw', 'thrown'), N('p1'), RES('p1', R('t1')), CATCH('c', 'p1', LA)]);
P('resolve-with-never-thenable', 'a thenable that never settles leaves the promise pending.', [TH('t1', 'never'), N('p1'), RES('p1', R('t1')), THEN('t', 'p1', LA), LOG('sync')]);
P('resolve-with-twice-thenable', 'a thenable that calls resolve twice: the first call wins.', [TH('t1', 'twice', 'first'), N('p1'), RES('p1', R('t1')), THEN('t', 'p1', LA)]);
P('first-resolution-wins', 'only the first resolve or reject takes effect.', [N('p1'), RES('p1', 'a'), RES('p1', 'b'), REJ('p1', 'c'), THEN('t', 'p1', LA)]);
P('reject-after-resolve-ignored', 'a reject after a resolve is ignored.', [N('p1'), REJ('p1', 'x'), RES('p1', 'y'), CATCH('c', 'p1', LA)]);
P('thenable-then-called-by-job', 'the then of a thenable runs in a job, not during resolve.', [TH('t1', 'resolve', 'z'), N('p1'), RES('p1', R('t1')), LOG('after-resolve'), THEN('t', 'p1', LA)]);
P('thenable-then-called-once-per-resolution', 'each resolution of a thenable calls its then once.', [TH('t1', 'resolve', 'once'), N('p1'), RES('p1', R('t1')), N('p2'), RES('p2', R('t1')), THEN('a', 'p1', LA), THEN('b', 'p2', LA)]);
P('static-resolve-keeps-promise-identity', 'Promise.resolve of a promise returns that promise.', [ST('q', 'resolve', ['z']), ST('r', 'resolve', [R('q')]), THEN('t', 'r', LA), THEN('u', 'q', LA)]);
P('static-reject-unhandled', 'Promise.reject with no handler is unhandled.', [ST('r', 'reject', ['nope'])]);
P('static-resolve-thenable', 'Promise.resolve of a thenable adopts it.', [TH('t1', 'resolve', 'adopted'), ST('p', 'resolve', [R('t1')]), THEN('t', 'p', LA), LOG('sync')]);
P('promise-of-promise-chain', 'a promise resolved with a promise resolved with a promise.', [ST('a', 'resolve', ['deep']), N('b'), RES('b', R('a')), N('c'), RES('c', R('b')), THEN('t', 'c', LA)]);
P('then-returns-thenable', 'a handler returning a thenable adopts it.', [TH('t1', 'resolve', 'th'), N('p1'), RES('p1', 1), THEN('t', 'p1', RT(R('t1'))), THEN('u', 't', LA)]);
P('resolve-deep-chain-of-adoptions', 'four adoptions in a row.', [ST('p0', 'resolve', ['d']), N('p1'), RES('p1', R('p0')), N('p2'), RES('p2', R('p1')), N('p3'), RES('p3', R('p2')), N('p4'), RES('p4', R('p3')), THEN('t', 'p4', LA)]);
P('resolve-with-number-and-string', 'numbers and strings are fulfilment values.', [N('p1'), RES('p1', 42), N('p2'), RES('p2', 'text'), THEN('a', 'p1', LA), THEN('b', 'p2', LA)]);
P('resolve-undefined', 'resolving with undefined fulfils with undefined.', [N('p1'), RES('p1', U), THEN('t', 'p1', LA), CATCH('c', 'p1', LG('no'))]);
P('resolve-after-adoption-pending', 'a second resolve after adopting a pending promise is ignored.', [N('q'), N('p1'), RES('p1', R('q')), RES('p1', 'direct'), THEN('t', 'p1', LA), RES('q', 'adopted')]);

// C. combinators
P('all-empty', 'Promise.all([]) fulfils at once with an empty array.', [ST('a', 'all', []), THEN('t', 'a', LA)]);
P('all-plain-values', 'Promise.all keeps the input order with plain values.', [ST('a', 'all', [1, 'two', 'three']), THEN('t', 'a', LA)]);
P('all-with-pending', 'Promise.all waits for pending inputs.', [N('p1'), N('p2'), ST('a', 'all', [R('p1'), R('p2')]), THEN('t', 'a', LA), RES('p2', 'second'), RES('p1', 'first')]);
P('all-first-rejection-in-time', 'Promise.all rejects with the first rejection in time, not the first in order.', [N('p1'), N('p2'), ST('a', 'all', [R('p1'), R('p2')]), CATCH('c', 'a', LA), REJ('p2', 'late-in-order'), REJ('p1', 'second')]);
P('all-with-thenable', 'Promise.all adopts thenables among its inputs.', [TH('t1', 'resolve', 'th'), ST('a', 'all', [R('t1'), 2]), THEN('t', 'a', LA)]);
P('all-settled-mixed', 'allSettled reports both outcomes in order.', [ST('a', 'allSettled', ['x']), N('p1'), REJ('p1', 'nope'), ST('b', 'allSettled', [R('p1'), 'y']), THEN('t', 'b', LA)]);
P('all-settled-empty', 'allSettled of nothing fulfils with an empty array.', [ST('a', 'allSettled', []), THEN('t', 'a', LA)]);
P('race-first-settled', 'race settles with the first settled input.', [N('p1'), N('p2'), ST('a', 'race', [R('p1'), R('p2')]), THEN('t', 'a', LA), RES('p2', 'two'), RES('p1', 'one')]);
P('race-value-beats-pending', 'race with a plain value settles with that value.', [N('p1'), ST('a', 'race', [R('p1'), 'value']), THEN('t', 'a', LA)]);
P('any-first-fulfilment', 'any settles with the first fulfilment.', [ST('bad', 'reject', ['a']), N('p2'), ST('a', 'any', [R('bad'), R('p2')]), THEN('t', 'a', LA), RES('p2', 'b')]);
P('any-all-rejected', 'any rejects with an AggregateError when every input rejects; a catch logs it.', [ST('x', 'reject', ['a']), ST('y', 'reject', ['b']), ST('a', 'any', [R('x'), R('y')]), CATCH('c', 'a', LG('aggregate'))]);
P('any-of-nothing', 'any of nothing rejects with an AggregateError.', [ST('a', 'any', []), CATCH('c', 'a', LG('empty'))]);
P('all-order-by-input', 'Promise.all resolves in input order even when later inputs settle first.', [N('p1'), N('p2'), N('p3'), ST('a', 'all', [R('p1'), R('p2'), R('p3')]), THEN('t', 'a', LA), RES('p3', 'c'), RES('p1', 'a'), RES('p2', 'b')]);
P('all-tick-count', 'Promise.all of two resolved promises: its observer runs after two extra ticks.', [ST('r1', 'resolve', [1]), ST('r2', 'resolve', [2]), ST('a', 'all', [R('r1'), R('r2')]), THEN('t', 'a', LG('ALL')), CHAIN_STEPS('d', ['a1', 'b1', 'c1', 'd1'])]);
P('all-settled-tick-count', 'allSettled observer timing against a four-step chain.', [ST('r', 'resolve', [1]), ST('a', 'allSettled', [R('r')]), THEN('t', 'a', LG('S')), CHAIN_STEPS('d', ['a1', 'b1', 'c1', 'd1'])]);
P('race-tick-count', 'race observer timing against a three-step chain.', [ST('r', 'resolve', [1]), ST('a', 'race', [R('r')]), THEN('t', 'a', LG('R')), CHAIN_STEPS('d', ['a1', 'b1', 'c1'])]);
P('any-tick-count', 'any observer timing against a four-step chain.', [ST('x', 'reject', [1]), ST('r', 'resolve', [2]), ST('a', 'any', [R('x'), R('r')]), THEN('t', 'a', LG('Y')), CHAIN_STEPS('d', ['a1', 'b1', 'c1', 'd1'])]);
P('all-of-all', 'nested combinators.', [ST('inner', 'all', [1, 2]), ST('outer', 'all', [R('inner'), 3]), THEN('t', 'outer', LA)]);
P('with-resolvers-then-resolve', 'withResolvers: the promise resolves when its resolver is called.', [WR('w'), THEN('t', 'w', LA), LOG('sync'), RES('w', 'ok')]);
P('with-resolvers-reject-unhandled', 'withResolvers: a rejection with no handler is unhandled.', [WR('w'), REJ('w', 'wr-lost')]);
P('all-thenable-rejects', 'Promise.all where a thenable rejects.', [TH('t1', 'reject', 'th-no'), ST('a', 'all', [R('t1'), 'ok']), CATCH('c', 'a', LA)]);
P('all-with-already-rejected-and-later-fulfil', 'all with a rejected input still rejects after other fulfilments.', [ST('bad', 'reject', ['early']), N('p'), ST('a', 'all', [R('bad'), R('p')]), CATCH('c', 'a', LA), RES('p', 'x')]);

// D. async functions
P('async-plain-return', 'an async call without await returns a fulfilled promise.', [ASYNC('a', [SR('v')]), THEN('t', 'a', LA)]);
P('async-await-resolved', 'await of a resolved promise costs one tick.', [ST('q', 'resolve', ['q']), ASYNC('a', [AW(R('q')), SL('after')]), CHAIN_STEPS('d', ['m1', 'm2'])]);
P('async-await-pending-later', 'await of a pending promise resumes after it resolves.', [N('q'), ASYNC('a', [AW(R('q')), SL('resumed'), SR('done')]), THEN('t', 'a', LA), RES('q', 'x')]);
P('async-await-value', 'await of a plain value costs one tick.', [ASYNC('a', [AW(5), SL('a')]), CHAIN_STEPS('d', ['m1', 'm2'])]);
P('async-await-undefined', 'await undefined resumes in the next job.', [ASYNC('a', [SL('before'), AW(U), SL('after')]), LOG('sync')]);
P('async-await-sync-thenable', 'await of a thenable that resolves synchronously.', [TH('t1', 'resolve', 'tv'), ASYNC('a', [AW(R('t1')), SL('got')]), CHAIN_STEPS('d', ['m1', 'm2', 'm3']), LOG('sync')]);
P('async-await-rejecting-thenable-caught', 'await of a rejecting thenable inside try/catch.', [TH('t1', 'reject', 'th-bad'), ASYNC('a', [TRY([AW(R('t1')), SL('no')], [CAUGHT]), SL('after')])]);
P('async-await-rejected-caught', 'await of a rejected promise inside try/catch, with finally.', [ST('q', 'reject', ['bad']), ASYNC('a', [TRY([AW(R('q')), SL('no')], [CAUGHT], [SL('fin')])]), CHAIN_STEPS('d', ['m1', 'm2'])]);
P('async-await-rejected-uncaught', 'an uncaught rejected await rejects the call; the call result is unhandled.', [ST('q', 'reject', ['lost-await']), ASYNC('a', [AW(R('q')), SL('no')])]);
P('async-return-promise', 'returning a promise from an async call adopts it.', [ST('q', 'resolve', ['P']), ASYNC('a', [SR(R('q'))]), THEN('t', 'a', LA), CHAIN_STEPS('d', ['a1', 'b1', 'c1', 'd1'])]);
P('async-throw-before-await', 'a throw before the first await rejects the call synchronously-looking promise.', [ASYNC('a', [SL('start'), SX('early')]), CATCH('c', 'a', LA), LOG('sync')]);
P('async-try-finally-normal', 'finally runs after a normal return.', [ASYNC('a', [TRY([AW(1), SL('body')], null, [SL('fin')]), SR('end')]), THEN('t', 'a', LA)]);
P('async-try-finally-throw', 'finally runs before the rejection is observed.', [ASYNC('a', [TRY([AW(1), SX('boom')], null, [SL('fin')])]), CATCH('c', 'a', LA)]);
P('async-nested-try', 'nested try blocks with catch and finally.', [N('bad'), REJ('bad', 'inner'), ASYNC('a', [TRY([TRY([AW(R('bad')), SL('no')], [CAUGHT], [SL('inner-fin')]), SL('between')], null, [SL('outer-fin')]), SR('ok')])]);
P('async-finally-awaits', 'await inside finally.', [ASYNC('a', [TRY([SL('body')], null, [AW(2), SL('fin-after-await')]), SR('r')]), THEN('t', 'a', LA), CHAIN_STEPS('d', ['m1', 'm2'])]);
P('async-catch-rethrow', 'a catch block that throws a new value.', [ST('q', 'reject', ['first']), ASYNC('a', [TRY([AW(R('q'))], [CAUGHT, SX('second')])]), CATCH('c', 'a', LA)]);
P('async-two-awaits-with-chain', 'two awaits interleave with a chain of thens.', [ASYNC('a', [AW(1), SL('a1'), AW(2), SL('a2')]), CHAIN_STEPS('d', ['m1', 'm2', 'm3'])]);
P('async-four-sequential-awaits', 'four awaits in one body keep their order.', [ASYNC('a', [AW(1), SL('s1'), AW(2), SL('s2'), AW(3), SL('s3'), AW(4), SL('s4')]), LOG('sync')]);
P('async-returns-awaited-promise', 'return of a promise that is awaited earlier.', [ST('q', 'resolve', ['qv']), ASYNC('a', [AW(R('q')), SR(R('q'))]), THEN('t', 'a', LA)]);
P('two-async-calls-interleave', 'two async calls interleave their awaits.', [ASYNC('a', [SL('a0'), AW(1), SL('a1')]), ASYNC('b', [SL('b0'), AW(1), SL('b1')])]);
P('async-logs-around-await', 'synchronous logs before the first await, then after.', [N('p'), ASYNC('a', [SL('one'), AW(R('p')), SL('two')]), LOG('sync-mid'), RES('p', 'x')]);
P('async-catch-logs-caught-value', 'catch sees the reason of the awaited rejection.', [ST('q', 'reject', ['caught-me']), ASYNC('a', [TRY([AW(R('q'))], [CAUGHT])]), THEN('t', 'a', LA)]);
P('async-return-inside-try-with-finally', 'return inside try runs finally, then the call resolves.', [ASYNC('a', [TRY([SR('early')], null, [SL('fin')]), SL('unreached')]), THEN('t', 'a', LA)]);
P('async-throw-in-catch-with-finally', 'a throw inside catch still runs finally.', [ASYNC('a', [TRY([SX('x1')], [SX('x2')], [SL('fin')])]), CATCH('c', 'a', LA)]);
P('async-await-then-log', 'await then then-handler: order of the awaiting continuation and the handler.', [ST('q', 'resolve', ['v']), ASYNC('a', [AW(R('q')), SL('cont')]), THEN('t', 'q', LG('handler')), LOG('sync')]);
P('async-result-with-finally', 'finally after an async call.', [ASYNC('a', [AW(1), SR('v')]), FIN('f', 'a', LG('fin')), THEN('t', 'f', LA)]);
P('async-await-chain-of-promises', 'await of a promise that itself is a then chain.', [N('p'), THEN('q', 'p', LA), ASYNC('a', [AW(R('q')), SL('after-q')]), RES('p', 'pv')]);

// E. unhandled rejections and queue markers
P('unhandled-simple', 'a rejection with no handler is reported at the end.', [N('p1'), REJ('p1', 'lost')]);
P('handled-before-drain', 'a handler attached before the drain: nothing is reported.', [ST('p1', 'reject', ['caught']), CATCH('c', 'p1', LA)]);
P('unhandled-order', 'two unhandled rejections are reported in rejection order.', [ST('a', 'reject', ['first']), ST('b', 'reject', ['second'])]);
P('late-handler-same-drain', 'a handler attached later in the same program still counts before the drain.', [ST('a', 'reject', ['late-handled']), LOG('mid'), CATCH('c', 'a', LA)]);
P('derived-unhandled-from-throw', 'a handler that throws leaves its derived promise unhandled.', [N('p1'), RES('p1', 1), THEN('t', 'p1', TW('derived'))]);
P('async-unhandled-result', 'an async call that throws without a handler is unhandled.', [ASYNC('a', [SX('async-lost')])]);
P('queue-and-log-order', 'queue markers and sync logs keep their order.', [Q('q1'), LOG('s1'), Q('q2'), LOG('s2')]);
P('queue-after-promise-jobs', 'a queue marker after a then reaction.', [N('p1'), RES('p1', 'x'), THEN('t', 'p1', LA), Q('q')]);
P('mixed-rejections-and-resolutions', 'some rejections are handled, some are not.', [ST('a', 'reject', ['r1']), ST('b', 'resolve', ['ok']), ST('c', 'reject', ['r2']), CATCH('h', 'a', LG('h1')), THEN('t', 'c', LA)]);
P('thenable-reject-unhandled', 'a thenable that rejects with no handler is unhandled.', [TH('t1', 'reject', 'th-lost'), N('p1'), RES('p1', R('t1'))]);
P('finally-unhandled-passthrough', 'finally passes a rejection through to an unhandled promise.', [ST('a', 'reject', ['through']), FIN('f', 'a', LG('f'))]);

// Tick programs: a resolved promise followed by a chain of thens that each log one label. The observer in the program
// before the chain shows how many ticks the observed promise took, relative to the chain (see tick-table.mjs).
function CHAIN_STEPS(prefix, labels) {
  const ops = [ST(`${prefix}0`, 'resolve', [U])];
  labels.forEach((label, i) => ops.push(THEN(`${prefix}${i + 1}`, `${prefix}${i}`, LG(label))));
  return ops;
}

// ----- seeded random programs (indices 101-400) -----
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function randomProgram(index) {
  const rnd = rng(1000 + index * 7919);
  const pick = (arr) => arr[Math.floor(rnd() * arr.length)];
  const ints = (n) => Math.floor(rnd() * n);
  const chance = (p) => rnd() < p;
  const labels = ['a', 'b', 'c', 'ok', 'bad', 'x1', 'y-2'];
  const promises = [];
  const resolvable = [];
  const thenables = [];
  let counter = 0;
  const fresh = (kind) => `${kind}${counter++}`;
  // Values that can be fulfilment values: strings, numbers, undefined, promises and thenables (never self).
  const value = (selfId) => {
    const r = rnd();
    if (r < 0.45) return pick(labels);
    if (r < 0.6) return 1 + ints(3);
    if (r < 0.66) return U;
    const pool = [...promises, ...thenables].filter((x) => x !== selfId);
    if (pool.length && r < 0.9) return R(pick(pool));
    return pick(labels);
  };
  // Reasons and thrown values: strings, numbers, undefined (no object values, so unhandled reasons render alike).
  const reason = () => {
    const r = rnd();
    if (r < 0.7) return pick(labels);
    if (r < 0.85) return 1 + ints(3);
    return U;
  };
  const handler = () => {
    const r = rnd();
    if (r < 0.3) return NONE;
    if (r < 0.5) return LG(pick(labels));
    if (r < 0.65) return LA;
    if (r < 0.82) return RT(value(null));
    return TW(reason());
  };
  const steps = (depth, n, inCatch) => {
    const out = [];
    for (let i = 0; i < n; i++) {
      const r = rnd();
      if (r < 0.35) out.push(AW(value(null)));
      else if (r < 0.55) out.push(SL(pick(labels)));
      else if (r < 0.62) out.push(SR(value(null)));
      else if (r < 0.68) out.push(SX(reason()));
      else if (r < 0.85 && depth < 2) {
        out.push(TRY(steps(depth + 1, 1 + ints(3), false), chance(0.7) ? steps(depth + 1, 1 + ints(2), true) : null, chance(0.4) ? steps(depth + 1, 1 + ints(2), false) : null));
      } else if (inCatch && r < 0.9) out.push(CAUGHT);
      else out.push(SL(pick(labels)));
    }
    return out;
  };
  const ops = [];
  const n = 3 + ints(12);
  for (let i = 0; i < n; i++) {
    const r = rnd();
    if (r < 0.12) {
      const id = fresh('p');
      ops.push(N(id));
      promises.push(id);
      resolvable.push(id);
    } else if (r < 0.16) {
      const id = fresh('p');
      ops.push(WR(id));
      promises.push(id);
      resolvable.push(id);
    } else if (r < 0.24 && resolvable.length) {
      const id = pick(resolvable);
      ops.push(chance(0.6) ? RES(id, value(id)) : REJ(id, reason()));
    } else if (r < 0.28) {
      const id = fresh('t');
      ops.push(TH(id, pick(['resolve', 'reject', 'throw', 'never', 'twice']), reason()));
      thenables.push(id);
    } else if (r < 0.42 && promises.length) {
      const id = fresh('p');
      const src = pick(promises);
      const k = rnd();
      if (k < 0.5) ops.push(THEN(id, src, handler(), handler()));
      else if (k < 0.75) ops.push(CATCH(id, src, handler()));
      else ops.push(FIN(id, src, handler()));
      promises.push(id);
    } else if (r < 0.52) {
      const id = fresh('p');
      const fn = pick(['all', 'allSettled', 'race', 'any', 'resolve', 'reject']);
      const items = fn === 'resolve' ? [value(null)] : fn === 'reject' ? [reason()] : Array.from({length: ints(4)}, () => value(null));
      ops.push(ST(id, fn, items));
      promises.push(id);
    } else if (r < 0.6) ops.push(LOG(pick(labels)));
    else if (r < 0.64) ops.push(Q(pick(labels)));
    else if (r < 0.8) {
      const id = fresh('p');
      ops.push(ASYNC(id, steps(0, 1 + ints(5), false)));
      promises.push(id);
    } else {
      const id = fresh('p');
      ops.push(N(id));
      promises.push(id);
      resolvable.push(id);
    }
  }
  return {name: `random-${String(index).padStart(3, '0')}`, doc: `seeded random program ${index}`, ops};
}

export function allPrograms() {
  const random = [];
  for (let i = 0; i < 300; i++) random.push(randomProgram(i));
  return [...HAND, ...random];
}

export async function generate() {
  const out = [];
  for (const p of allPrograms()) {
    const expect = await runNode(p);
    out.push({name: p.name, doc: p.doc, ops: p.ops, expect});
  }
  return out;
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  const programs = await generate();
  writeFileSync(outFile, '[\n' + programs.map((p) => JSON.stringify(p)).join(',\n') + '\n]\n');
  console.log(`wrote ${programs.length} programs (${HAND.length} hand-designed) to ${path.relative(process.cwd(), outFile)}`);
}
