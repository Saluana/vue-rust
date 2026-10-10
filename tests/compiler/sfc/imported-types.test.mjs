// Imported types in <script setup lang="ts"> (task 5.7). The official compiler (compileScript of @vue/compiler-sfc 3.5.43)
// reads an imported type file to derive a prop's runtime type. Compiled here, that read is limited to the project root
// (SfcContext.scriptFs), and a relative type import that names no file is VN6017 instead of an untyped prop. The expected
// props are hand-written from Vue's production rules: only Boolean and Function keep a type, defaults are kept, and
// `required` is dropped. The official compiler's output for the same files (absolute names) is the second reference.
import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { createRequire } from 'node:module';
import { compileSfc } from '../../../compiler/sfc/index.mjs';
import { FIXTURES, ROOT, eraseTypes, moduleToFunction } from './harness.mjs';
import { writeProject } from '../../scratch.mjs';

const require = createRequire(import.meta.url);
const { parse, compileScript } = require('vue/compiler-sfc');
const Vue = require('vue');
const { renderToString } = require('vue/server-renderer');

// The props object literal in compiled code, brace matched, with whitespace collapsed (null when there is none).
function propsOf(code) {
  if (typeof code !== 'string') return null;
  const start = code.indexOf('props: {');
  if (start < 0) return null;
  let depth = 0;
  for (let i = start + 'props: '.length; i < code.length; i++) {
    if (code[i] === '{') depth++;
    else if (code[i] === '}' && --depth === 0) return code.slice(start, i + 1).replace(/\s+/g, ' ');
  }
  return null;
}

// file:line:column-line:column and code for each diagnostic.
const spans = (r) => r.diagnostics.map((d) => `${d.code} ${d.start.line}:${d.start.column}-${d.end.line}:${d.end.column}`);

// File access for the official compiler without a root limit: absolute names, the reference behavior.
const unrestrictedFs = {
  fileExists: (file) => fs.existsSync(file) && fs.statSync(file).isFile(),
  readFile: (file) => fs.readFileSync(file, 'utf8'),
  realpath: (file) => fs.realpathSync(file),
};

// The props the official compiler derives for a component file, given by absolute path.
function officialProps(file) {
  const { descriptor, errors } = parse(fs.readFileSync(file, 'utf8'), { filename: file });
  assert.deepEqual(errors, []);
  return propsOf(compileScript(descriptor, { id: 'oracle', isProd: true, fs: unrestrictedFs }).content);
}

// Compiles a component file with the same options the frontend uses for a project.
function compileAt(rootDir, file) {
  return compileSfc(fs.readFileSync(path.join(rootDir, file), 'utf8'), { filename: file, rootDir });
}

const EXPECTED_PROPS = {
  'ok-imported-props.vue': 'props: { label: {}, count: {}, enabled: { type: Boolean } }',
  'ok-imported-alias.vue': "props: { title: { default: 'Untitled' }, size: { default: 2 }, open: { type: Boolean }, tags: {} }",
};

describe('imported prop types resolve to the production runtime props', () => {
  for (const [file, expected] of Object.entries(EXPECTED_PROPS)) {
    test(`${file}: no diagnostics, the props of the imported type, and the type file is a dependency`, () => {
      const r = compileAt(FIXTURES, file);
      assert.deepEqual(r.diagnostics, []);
      assert.equal(typeof r.code, 'string');
      assert.equal(propsOf(r.code), expected);
      assert.ok(r.dependencies.includes('types-imported.ts'), JSON.stringify(r.dependencies));
    });

    test(`${file}: the props equal the official compileScript output for the same file`, () => {
      const official = officialProps(path.join(FIXTURES, file));
      assert.equal(official, expected);
      assert.equal(propsOf(compileAt(FIXTURES, file).code), official);
    });
  }
});

describe('the working directory does not matter', () => {
  test('the same component compiles to the same output from another working directory', () => {
    const before = compileAt(FIXTURES, 'ok-imported-alias.vue').code;
    const cwd = process.cwd();
    process.chdir(path.parse(ROOT).root);
    try {
      assert.equal(compileAt(FIXTURES, 'ok-imported-alias.vue').code, before);
    } finally {
      process.chdir(cwd);
    }
  });
});

describe('type files are read only inside the project root', () => {
  test('a type import of a file outside the root is VN6017 at the module name, where the official compiler would give an untyped prop', () => {
    const r = compileAt(FIXTURES, 'bad-import-type-outside.vue');
    // Line 2 is `import type { Outside } from '../outside.ts'`; the module name starts at column 29 and is 15 characters.
    assert.deepEqual(spans(r), ['VN6017 2:29-2:44']);
    assert.equal(r.code, null);
    assert.deepEqual(r.dependencies, []);
    assert.equal(officialProps(path.join(FIXTURES, 'bad-import-type-outside.vue')), 'props: { flag: {} }');
  });

  test('negative control: a type import whose file is missing is VN6017; adding the file clears it', () => {
    const dir = writeProject('imported-missing', {
      'Comp.vue': fs.readFileSync(path.join(FIXTURES, 'bad-import-type-missing.vue'), 'utf8'),
    });
    // Line 2 is `import type { Missing } from './types-missing.ts'`; the module name starts at column 29.
    const missing = compileAt(dir, 'Comp.vue');
    assert.deepEqual(spans(missing), ['VN6017 2:29-2:49']);
    assert.equal(missing.code, null);
    fs.writeFileSync(path.join(dir, 'types-missing.ts'), 'export type Missing = boolean\n');
    const present = compileAt(dir, 'Comp.vue');
    assert.deepEqual(present.diagnostics, []);
    assert.equal(propsOf(present.code), 'props: { flag: { type: Boolean } }');
    assert.deepEqual(present.dependencies, ['types-missing.ts']);
  });

  test('a file inside the root is read when the component is in a subdirectory of it', () => {
    const r = compileSfc(fs.readFileSync(path.join(FIXTURES, 'bad-import-type-outside.vue'), 'utf8'), {
      filename: 'fixtures/bad-import-type-outside.vue',
      rootDir: path.join(ROOT, 'tests', 'sfc'),
    });
    assert.deepEqual(r.diagnostics, []);
    assert.deepEqual(r.dependencies, ['outside.ts']);
    assert.equal(propsOf(r.code), 'props: { flag: {} }');
  });
});

// Renders a compiled fixture with Vue's server renderer (vue@3.5.43, run here, no browser needed). `mutation` is an optional
// [from, to] text replacement applied to the compiled module, for the negative controls.
async function renderFixture(file, props, mutation = null) {
  const r = compileAt(FIXTURES, file);
  assert.deepEqual(r.diagnostics, []);
  let js = r.lang === 'ts' ? eraseTypes(r.code) : r.code;
  if (mutation) {
    assert.ok(js.includes(mutation[0]), `the mutation applies: ${mutation[0]}`);
    js = js.replace(mutation[0], mutation[1]);
  }
  const component = vm.runInThisContext(moduleToFunction(js))(Vue);
  return renderToString(Vue.createSSRApp({ render: () => Vue.h(component, props) }));
}

describe('the compiled props behave as Vue defines them (server rendering with vue@3.5.43)', () => {
  test('ok-imported-props.vue: the values passed in are rendered, and an absent boolean is false', async () => {
    assert.equal(await renderFixture('ok-imported-props.vue', { label: 'a', count: 2, enabled: true }), '<p>a 2 on</p>');
    assert.equal(await renderFixture('ok-imported-props.vue', { label: 'b', count: 0 }), '<p>b 0 off</p>');
  });

  test('ok-imported-alias.vue: withDefaults supplies Untitled and 2; explicit values replace them', async () => {
    assert.equal(await renderFixture('ok-imported-alias.vue', {}), '<h2>Untitled 2 closed</h2>');
    assert.equal(await renderFixture('ok-imported-alias.vue', { title: 'Hi', size: 5, open: true }), '<h2>Hi 5 open</h2>');
  });

  test('negative control: a props definition without label renders differently (label falls through to the element)', async () => {
    const rendered = await renderFixture('ok-imported-props.vue', { label: 'a', count: 2, enabled: true }, ['label: {},', '']);
    assert.notEqual(rendered, '<p>a 2 on</p>');
  });

  test('negative control: a changed default in the compiled props renders differently', async () => {
    const rendered = await renderFixture('ok-imported-alias.vue', {}, ["default: 'Untitled'", "default: 'Other'"]);
    assert.notEqual(rendered, '<h2>Untitled 2 closed</h2>');
  });
});
