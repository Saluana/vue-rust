// Prints the tick-count table of WP-B5: for each named corpus program, V8's log (run live with the gen-b5 oracle) and the
// number of steps of an independent chain (a0..d0 of Promise.resolve().then(...)) that ran before the observer. Each chain
// step is one job, so the count is the number of ticks the observer waited behind the chain.
// Usage: node tests/language/promise-programs/tick-table.mjs
import {allPrograms, runNode} from '../gen/gen-b5.mjs';

const cases = [
  {name: 'all-tick-count', observer: 'ALL', chain: ['a1', 'b1', 'c1', 'd1'], what: 'Promise.all of two resolved promises'},
  {name: 'all-settled-tick-count', observer: 'S', chain: ['a1', 'b1', 'c1', 'd1'], what: 'allSettled of one resolved promise'},
  {name: 'race-tick-count', observer: 'R', chain: ['a1', 'b1', 'c1'], what: 'race of one resolved promise'},
  {name: 'any-tick-count', observer: 'Y', chain: ['a1', 'b1', 'c1', 'd1'], what: 'any: one rejected, one fulfilled'},
  {name: 'finally-chains-ticks', observer: 'F', chain: ['a', 'b', 'c', 'd', 'e'], what: 'finally on a fulfilled promise'},
  {name: 'async-await-resolved', observer: 'after', chain: ['m1', 'm2'], what: 'await of a resolved promise'},
  {name: 'async-await-value', observer: 'a', chain: ['m1', 'm2'], what: 'await of a plain value'},
  {name: 'async-return-promise', observer: 'P', chain: ['a1', 'b1', 'c1', 'd1'], what: 'async return of a resolved promise'},
  {name: 'async-two-awaits-with-chain', observer: 'a2', chain: ['m1', 'm2', 'm3'], what: 'second await of an async body'},
  {name: 'async-await-sync-thenable', observer: 'got', chain: ['m1', 'm2', 'm3'], what: 'await of a synchronous thenable'},
];

const byName = new Map(allPrograms().map((p) => [p.name, p]));
let failures = 0;
console.log('program                          observer  chain steps before it  V8 log');
for (const c of cases) {
  const prog = byName.get(c.name);
  if (!prog) throw new Error(`no program named ${c.name}`);
  const {log} = await runNode(prog);
  const at = log.indexOf(c.observer);
  if (at < 0) throw new Error(`${c.name}: observer ${c.observer} was not logged`);
  const before = log.slice(0, at).filter((e) => c.chain.includes(e)).length;
  console.log(`${c.name.padEnd(32)} ${c.observer.padEnd(9)} ${String(before).padEnd(22)} ${log.join(' | ')}`);
  if (log.length === 0) failures++;
}
if (failures) process.exitCode = 1;
