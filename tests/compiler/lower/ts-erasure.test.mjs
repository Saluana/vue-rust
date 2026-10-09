// TypeScript erasure (WP-C1c): type-only syntax lowers to the same Rust as its JavaScript (no runtime trace of a type),
// enums stay rejected, and type-only imports are elided while value imports keep their request.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compileModule } from '../../../compiler/index.mjs';

const compile = (src, lang = 'ts') => compileModule(src, { filename: 'erasure.ts', lang, specifier: '/erasure.ts' });

test('annotations, as, satisfies, non-null, generic calls, optional parameters lower to the JavaScript meaning', () => {
  const ts = [
    'interface Shape { w: number }',
    'type Id = string | number',
    'function pick<T>(xs: T[], i?: number): T | undefined {',
    '  return xs[i ?? 0] as T | undefined',
    '}',
    'const cfg = { n: 2 } satisfies Shape & { n: number }',
    'let box: Shape | null = { w: 3 }',
    'export const total = (box!.w as number) + cfg.n + pick<number>([1, 2], 1)!',
    'export type Alias = Id',
    'export function label(x: Id): string { return String(x) }',
    '',
  ].join('\n');
  const js = [
    'function pick(xs, i) {',
    '  return xs[i ?? 0]',
    '}',
    'const cfg = { n: 2 }',
    'let box = { w: 3 }',
    'export const total = (box.w) + cfg.n + pick([1, 2], 1)',
    'export function label(x) { return String(x) }',
    '',
  ].join('\n');
  const a = compile(ts);
  assert.ok(a.rust !== null, JSON.stringify(a.diagnostics.map((d) => d.message)));
  const b = compile(js, 'js');
  assert.ok(b.rust !== null);
  assert.deepEqual(a.requests, b.requests);
});

test('enums are rejected (VN3001), not erased', () => {
  const c = compile('enum Color { Red, Green }\nexport const c = Color.Red\n');
  assert.equal(c.rust, null);
  assert.ok(c.diagnostics.some((d) => d.code === 'VN3001' && d.severity === 'error'));
});

test('a type-only import is elided: no request, no binding', () => {
  const c = compile("import { Shape } from './types.ts'\nlet s: Shape | null = null\nexport const n = s\n");
  assert.ok(c.rust !== null, JSON.stringify(c.diagnostics.map((d) => d.message)));
  assert.deepEqual(c.requests, []);
  assert.doesNotMatch(c.rust, /module_import_cell/);
});

test('negative control: a value use keeps the import and its request', () => {
  const c = compile("import { make } from './types.ts'\nexport const n = make()\n");
  assert.ok(c.rust !== null);
  assert.deepEqual(c.requests, ['./types.ts']);
  assert.match(c.rust, /module_import_cell\(md, "\.\/types\.ts", "make"\)/);
});

test('import type and export type are erased whole; a side-effect import is kept', () => {
  const c = compile("import type { T } from './t.ts'\nexport type { T as U } from './t.ts'\nimport './effects.ts'\nexport const x = 1\n");
  assert.ok(c.rust !== null, JSON.stringify(c.diagnostics.map((d) => d.message)));
  assert.deepEqual(c.requests, ['./effects.ts']);
});
