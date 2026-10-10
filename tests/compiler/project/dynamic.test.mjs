// Unit tests for dynamic import() in the project compiler (WP-C1c.5.5): a literal target joins the project graph, and
// every rejected form has its code and a span at the offending text. The literal forms of the same graph are the
// negative controls: they compile, so the rejects are not caused by the graph itself.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compileProject } from '../../../compiler/project/index.mjs';

const errors = (r) => r.diagnostics.filter((d) => d.severity === 'error');
const codes = (r) => errors(r).map((d) => d.code);
const messages = (r) => errors(r).map((d) => d.message);
// main.js is the entry; x.js is a plain module that the literal forms import.
const compile = (src, extra = {}) =>
  compileProject({ entry: 'main.js', files: { 'main.js': src, 'x.js': 'export const x = 1\n', ...extra }, target: 'test' });

test('a literal target joins the graph and is not a static request (negative control)', () => {
  const r = compile("export const run = () => import('./x.js')\n");
  assert.equal(r.ok, true, messages(r).join(' | '));
  assert.deepEqual(r.modules.map((m) => m.id), ['/main.js', '/x.js']);
  assert.deepEqual(r.modules.find((m) => m.id === '/main.js').requests, []);
});

test('a no-substitution template is a literal specifier', () => {
  const r = compile('export const run = () => import(`./x.js`)\n');
  assert.equal(r.ok, true, messages(r).join(' | '));
  assert.deepEqual(r.modules.map((m) => m.id), ['/main.js', '/x.js']);
});

test('the targets of dynamic imports are compiled transitively', () => {
  const r = compileProject({
    entry: 'main.js',
    files: {
      'main.js': "export const run = () => import('./x.js')\n",
      'x.js': "export const f = () => import('./y.js')\n",
      'y.js': 'export const y = 2\n',
    },
    target: 'test',
  });
  assert.equal(r.ok, true, messages(r).join(' | '));
  assert.deepEqual(r.modules.map((m) => m.id), ['/main.js', '/x.js', '/y.js']);
});

test('a computed specifier is VN2019 at the argument', () => {
  const src = "const name = './x.js'\nexport const run = () => import(name)\n";
  const r = compile(src);
  assert.deepEqual(codes(r), ['VN2019']);
  assert.equal(errors(r)[0].start.offset, src.indexOf('import(name)') + 'import('.length);
});

test('a template with a substitution is VN2019 (computed specifier)', () => {
  const r = compile('const name = "x"\nexport const run = () => import(`./${name}.js`)\n');
  assert.deepEqual(codes(r), ['VN2019']);
});

test('import attributes or options are VN2031 at the second argument', () => {
  const opts = "{ with: { type: 'json' } }";
  const src = `export const run = () => import('./x.js', ${opts})\n`;
  const r = compile(src);
  assert.deepEqual(codes(r), ['VN2031']);
  assert.equal(errors(r)[0].start.offset, src.indexOf(opts));
  assert.equal(errors(r)[0].end.offset, src.indexOf(opts) + opts.length);
});

test('a missing relative target is VN9001 at the specifier, as for a static import', () => {
  const src = "export const run = () => import('./nope.js')\n";
  const r = compile(src);
  assert.deepEqual(codes(r), ['VN9001']);
  assert.match(messages(r)[0], /not in the project/);
  assert.equal(errors(r)[0].start.offset, src.indexOf("'./nope.js'"));
  const staticForm = compile("import('./x.js')\nexport const k = 1\nimport { q } from './nope.js'\nexport const z = q\n");
  assert.deepEqual(codes(staticForm), ['VN9001'], 'the static form reports the same code');
});

test('a directory-like relative target that is not a project file is VN9001', () => {
  const r = compile("export const run = () => import('./dir')\n");
  assert.deepEqual(codes(r), ['VN9001']);
  assert.match(messages(r)[0], /not in the project/);
});

test('a package specifier is VN9001 (closed world)', () => {
  const r = compile("export const run = () => import('lodash')\n");
  assert.deepEqual(codes(r), ['VN9001']);
  assert.match(messages(r)[0], /outside the closed world/);
});

test('an absolute specifier is VN9001', () => {
  const r = compile("export const run = () => import('/abs/x.js')\n");
  assert.deepEqual(codes(r), ['VN9001']);
  assert.match(messages(r)[0], /absolute module specifier/);
});

test('a native module is VN9001: its exports are bound by static imports only', () => {
  const r = compile("export const run = () => import('@vue-native/platform')\n");
  assert.deepEqual(codes(r), ['VN9001']);
  assert.match(messages(r)[0], /native module/);
});

test('import() of vue is VN6202 at the specifier (the vue namespace policy)', () => {
  const src = "export const run = () => import('vue')\n";
  const r = compile(src);
  assert.deepEqual(codes(r), ['VN6202']);
  assert.equal(errors(r)[0].start.offset, src.indexOf("'vue'"));
});

test('import() of a vue subpath is VN6203 and of a node builtin is VN6204', () => {
  assert.deepEqual(codes(compile("export const run = () => import('vue/server-renderer')\n")), ['VN6203']);
  assert.deepEqual(codes(compile("export const run = () => import('node:fs')\n")), ['VN6204']);
});

test('calls with zero arguments or a spread argument are parse errors (VN1000)', () => {
  assert.deepEqual(codes(compile('export const run = () => import()\n')), ['VN1000']);
  assert.deepEqual(codes(compile('export const run = (a) => import(...a)\n')), ['VN1000']);
});
