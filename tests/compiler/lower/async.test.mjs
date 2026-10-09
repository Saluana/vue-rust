// Async lowering (WP-C1c): the shape of emitted async code. Every Value of an async body lives in the heap frame (no
// Value temporary is a Rust local, so the collector sees it across an await); sync modules carry no async runtime import;
// async syntax outside the profile is rejected with its code.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compileModule } from '../../../compiler/index.mjs';

const compile = (src) => compileModule(src, { filename: 'async.js', lang: 'js', specifier: '/async.js' });

const ASYNC_SRC = [
  'function v(x) { return Promise.resolve(x) }',
  'export async function run(a) {',
  '  const parts = [a, await v(1)]',
  '  let total = 0',
  '  for (const p of parts) { total += await v(p) }',
  '  try { total += await v(1) } finally { total += 1 }',
  '  switch (await v(total)) { case 3: return total; default: return -1 }',
  '}',
  '',
].join('\n');

test('an async module imports the async runtime and emits async bodies with heap frames', () => {
  const c = compile(ASYNC_SRC);
  assert.ok(c.rust !== null, JSON.stringify(c.diagnostics.map((d) => d.message)));
  assert.match(c.rust, /use crate::vn_async::\{async_start, await_value, frame_get, frame_set, new_frame\};/);
  assert.match(c.rust, /async fn f\d+_run_body\(rt_: Rt, fr: ObjRef\) -> Res<Value>/);
  assert.match(c.rust, /Ok\(async_start\(rt, fr, Box::pin\(f\d+_run_body\(rt\.clone\(\), fr\)\)\)\)/);
});

test('no Value temporary or binding is a Rust local in an async body (GC safety by construction)', () => {
  const c = compile(ASYNC_SRC);
  const bodies = [...c.rust.matchAll(/async fn [\s\S]*?\n\}\n/g)].map((m) => m[0]);
  assert.equal(bodies.length, 1, 'one async function in the module');
  for (const body of bodies) {
    assert.doesNotMatch(body, /let (mut )?t\d+: Value\b/);
    assert.doesNotMatch(body, /let (mut )?v\d+_\w*: Value\b/);
    assert.doesNotMatch(body, /let (mut )?c\d+ = rt\.new_cell/);
  }
});

test('a sync module has no async runtime import and no frames (sync output is unchanged)', () => {
  const c = compile('export function run(a) { let t = a + 1; return t * 2 }\n');
  assert.ok(c.rust !== null);
  assert.doesNotMatch(c.rust, /vn_async/);
  assert.doesNotMatch(c.rust, /new_frame/);
});

test('async syntax outside the profile is rejected with a code: await in parameters, for await, async generators', () => {
  assert.ok(compile('export async function run(a = await 1) { return a }\n').diagnostics.some((d) => d.code === 'VN1000'));
  const fa = compile('export async function run() { for await (const x of [1]) { trace("x", x) } }\n');
  assert.equal(fa.rust, null);
  assert.ok(fa.diagnostics.some((d) => d.code === 'VN2010'));
  const gen = compile('export async function* run() { yield 1 }\n');
  assert.equal(gen.rust, null);
  assert.ok(gen.diagnostics.some((d) => d.code === 'VN2008'));
});

test('negative control: the same module without await compiles without frames', () => {
  const c = compile('export function run(a) { return [a, a] }\n');
  assert.doesNotMatch(c.rust, /frame_set/);
});
