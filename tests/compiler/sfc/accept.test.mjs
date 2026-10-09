// Accepting fixtures (WP-F1): every ok-*.vue compiles with no error, its output parses as a module, imports only names
// the profile allows for generated code, and its records (styles, bindings, dependencies, scope id) have the expected
// shape. The expectations here are hand-written from the fixture sources.
import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { compileSfc } from '../../../compiler/sfc/index.mjs';
import { parseModule } from '../../../compiler/frontend/parse.mjs';
import { isSupported } from '../../../compiler/profile/index.mjs';
import { FIXTURES } from './harness.mjs';

const OK = fs.readdirSync(FIXTURES).filter((f) => f.startsWith('ok-') && f.endsWith('.vue')).sort();

function compile(file, extra = {}) {
  const src = fs.readFileSync(path.join(FIXTURES, file), 'utf8');
  return compileSfc(src, { filename: file, rootDir: FIXTURES, ...extra });
}

describe('accepting fixtures', () => {
  test('there are at least 25 accepting fixtures', () => {
    assert.ok(OK.length >= 25, `have ${OK.length}`);
  });

  for (const file of OK) {
    test(`${file} compiles without errors to a module the frontend parses`, () => {
      const r = compile(file);
      assert.deepEqual(r.diagnostics.filter((d) => d.severity === 'error').map((d) => d.code), []);
      assert.equal(typeof r.code, 'string');
      const parsed = parseModule(r.code, { filename: file, lang: r.lang });
      assert.deepEqual(parsed.diagnostics.map((d) => d.code), [], 'generated module parses');
      assert.match(r.code, /^export default __sfc;\n$/m, 'the module ends with the component as default export');
      assert.match(r.scopeId, /^data-v-[0-9a-f]{8}$/, 'default scope id is data-v- plus 8 hex digits');
    });

    test(`${file}: generated imports from vue are allowed for generated code`, () => {
      const r = compile(file);
      const { ast } = parseModule(r.code, { filename: file, lang: r.lang });
      for (const node of ast.program.body) {
        if (node.type !== 'ImportDeclaration') continue;
        if (node.source.value === 'vue') {
          for (const spec of node.specifiers) {
            const name = spec.imported?.name ?? spec.imported?.value;
            if (spec.type === 'ImportDefaultSpecifier' || spec.type === 'ImportNamespaceSpecifier') continue;
            assert.ok(isSupported('vue', name, 'native', { origin: 'compiler' }), `${name} is allowed in generated code`);
          }
        }
      }
    });
  }

  test('a setup component exposes its bindings and the official __name', () => {
    const r = compile('ok-counter.vue');
    assert.equal(r.bindings.count, 'setup-ref');
    assert.equal(r.lang, 'js');
    assert.match(r.code, /__name: 'ok-counter'/);
  });

  test('TypeScript setup components are emitted as TypeScript', () => {
    assert.equal(compile('ok-with-defaults.vue').lang, 'ts');
  });

  test('a scoped style sets __scopeId and records the scope', () => {
    const r = compile('ok-scoped-style.vue', { id: 'fx-scoped' });
    assert.equal(r.scopeId, 'data-v-fx-scoped');
    assert.match(r.code, /__sfc\.__scopeId = "data-v-fx-scoped";/);
    assert.equal(r.styles.length, 1);
    assert.deepEqual(
      { scoped: r.styles[0].scoped, scopeId: r.styles[0].scopeId, module: r.styles[0].module, index: r.styles[0].index },
      { scoped: true, scopeId: 'data-v-fx-scoped', module: null, index: 0 },
    );
    assert.match(r.styles[0].code, /\.s\[data-v-fx-scoped\]/);
  });

  test('v-bind() in CSS is collected per style block', () => {
    const r = compile('ok-v-bind-css.vue');
    assert.deepEqual(r.styles[0].vars, ['color']);
    assert.match(r.styles[0].code, /var\(--/);
    assert.equal(r.styles[0].scopeId, null);
  });

  test('a style block without scoped records no scope id and no vars', () => {
    const r = compile('ok-normal-template.vue');
    assert.deepEqual(r.styles, []);
    assert.equal(r.map, null, 'non-setup components have no source map (documented)');
    assert.deepEqual(r.bindings, {});
  });

  test('a template src and a style src are recorded as dependencies', () => {
    assert.deepEqual(compile('ok-src-template.vue').dependencies, ['partials/body.html']);
    assert.deepEqual(compile('ok-style-src.vue').dependencies, ['partials/style.css']);
    assert.equal(compile('ok-style-src.vue').styles[0].filename, 'partials/style.css');
  });

  test('a setup component with a template exports the component as default and has no style import', () => {
    const r = compile('ok-scoped-style.vue');
    assert.ok(!/import\s[^;\n]*\.css/.test(r.code));
    assert.match(r.code, /export default __sfc;/);
  });

  test('a setup component source map names the file by its project-relative name', () => {
    const r = compile('ok-counter.vue');
    assert.deepEqual(r.map.sources, ['ok-counter.vue']);
  });
});
