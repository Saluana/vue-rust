// Callback-preset fixture generator (WP-T1, builtin corpora). For each sequence of calls it runs the presets of
// tooling/oracle/builtin-corpus.mjs (makePresets) in Node and writes one JSON line:
//   {"preset":name, "arity":[n per call], "pre":<encodeAll(flat args)>,
//    "post":{"throws":[null|name per call], "state":<encodeAll([...rets, ...flatArgs, log])>}}
// (rets are null for a throwing call, as the replay's finish() does). crates/testkit/tests/presets.rs replays the
// same sequences natively and must produce the same post. Usage: node presets-check.mjs [--check]
import {readFileSync, writeFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {makePresets} from '../../../tooling/oracle/builtin-corpus.mjs';
import {createRecorder} from '../../../tooling/oracle/trace.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const fixture = path.join(here, 'presets.jsonl');

const sym = Symbol('x');
const sequences = [
  {preset: 'identity', calls: [[1, 0], ['a', 2], [{}, 3], [{}], [null, 1], [undefined, 2], [true, 'x']]},
  {preset: 'double', calls: [[21], ['3'], [true], [null], [undefined], [{}, 4], [[1], 0]]},
  {preset: 'isEven', calls: [[2], [3], [-0, 1], [NaN], ['4'], [{}, 7], [1.5]]},
  {preset: 'isTruthy', calls: [[0], [''], ['x'], [{}], [null], [undefined, 1], [-0], [NaN]]},
  {preset: 'index', calls: [[10, 0], ['q', 3], [{}, 2], [{}], [undefined, 1]]},
  {preset: 'pair', calls: [[7, 1], [{}, 2], ['s'], [null, 5]]},
  {preset: 'sum', calls: [[1, 2, 0], ['a', 'b', 1], [1, '2', 2], [{}, 1, 0], [[1], [2], 3], [undefined, 1, 0]]},
  {preset: 'concat', calls: [[[1, 2], [3], 0], [1, [2, [3]], 0], [[, 2], [1], 1], ['a', 'b', 2], [{}, [], 0]]},
  {preset: 'cmpNum', calls: [[3, 1], [1, 3], [2, 2], ['10', 9], [NaN, 1], [{}, 1]]},
  {preset: 'cmpDesc', calls: [[3, 1], [1, 3], ['2', 2], [undefined, 4]]},
  {preset: 'cmpStr', calls: [['b', 'a'], ['a', 'b'], ['a', 'a'], [10, 9], ['10', 9], [NaN, 'x'], [sym, 'a']]},
  {preset: 'cmpThrow', calls: [[3, 1], [1, 3], [2, 2]]},
  {preset: 'throwAt2', calls: [['x', 0], ['y', 1], ['z', 2], ['w', 3]]},
  {preset: 'toArr2', calls: [[1], [{}], ['s'], [undefined]]},
  {preset: 'keyOf', calls: [[1], [-0], [null], [sym], [{}], [true], [undefined, 3]]},
];

function run(seq) {
  const flat = seq.calls.flat(1);
  const pre = createRecorder().encodeAll(flat);
  const log = [];
  const presets = makePresets(log);
  const rets = [];
  const throws = [];
  for (const args of seq.calls) {
    try {
      rets.push(presets[seq.preset](...args));
      throws.push(null);
    } catch (e) {
      rets.push(null);
      throws.push(typeof e?.name === 'string' ? e.name : null);
    }
  }
  const state = createRecorder().encodeAll([...rets, ...flat, log]);
  return JSON.stringify({
    preset: seq.preset,
    arity: seq.calls.map((c) => c.length),
    pre,
    post: {throws, state},
  });
}

const text = sequences.map((seq) => `${run(seq)}\n`).join('');
if (process.argv.includes('--check')) {
  let committed = null;
  try {
    committed = readFileSync(fixture, 'utf8');
  } catch {
    // a missing fixture is stale
  }
  if (committed !== text) {
    process.stderr.write('presets.jsonl is stale: run node tests/language/rust-trace/presets-check.mjs\n');
    process.exitCode = 1;
  } else {
    process.stdout.write(`presets.jsonl is current (${sequences.length} sequences)\n`);
  }
} else {
  writeFileSync(fixture, text);
  process.stdout.write(`wrote ${sequences.length} preset sequences to presets.jsonl\n`);
}
