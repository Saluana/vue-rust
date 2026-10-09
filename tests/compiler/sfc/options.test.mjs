// Options, targets and path security (WP-F1). Expected messages are the ones the code gives for these inputs.
import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { compileSfc, SFC_CODES, sfcCodeInfo } from '../../../compiler/sfc/index.mjs';
import { createAnyDiagnostic } from '../../../compiler/sfc/codes.mjs';
import { FIXTURES } from './harness.mjs';

const read = (f) => fs.readFileSync(path.join(FIXTURES, f), 'utf8');

describe('options', () => {
  test('filename is required and must be a string', () => {
    assert.throws(() => compileSfc('<template></template>', {}), TypeError);
    assert.throws(() => compileSfc('<template></template>', { filename: '' }), TypeError);
  });

  test('target must be native or web', () => {
    assert.throws(() => compileSfc('<template></template>', { filename: 'a.vue', target: 'desktop' }), TypeError);
  });

  test('id must be letters, digits, underscore or dash', () => {
    assert.throws(() => compileSfc('<template></template>', { filename: 'a.vue', id: 'a b' }), TypeError);
    assert.throws(() => compileSfc('<template></template>', { filename: 'a.vue', id: '' }), TypeError);
  });

  test('trustedHtml is only allowed for the web target', () => {
    assert.throws(() => compileSfc('<template></template>', { filename: 'a.vue', trustedHtml: true }), TypeError);
  });

  test('a missing rootDir is an error, not a silent default', () => {
    assert.throws(() => compileSfc('<template></template>', { filename: 'a.vue', rootDir: '/no/such/root/dir' }), /project root does not exist/);
  });

  test('the source must be a string', () => {
    assert.throws(() => compileSfc(42, { filename: 'a.vue' }), TypeError);
  });

  test('explicit id is used for the scope id and the style attribute', () => {
    const r = compileSfc(read('ok-scoped-style.vue'), { filename: 'ok-scoped-style.vue', rootDir: FIXTURES, id: 'abc-1' });
    assert.equal(r.scopeId, 'data-v-abc-1');
    assert.equal(r.styles[0].scopeId, 'data-v-abc-1');
  });
});

describe('targets', () => {
  test('v-html is rejected on native, and accepted on web only with trustedHtml', () => {
    const src = '<template><p v-html="raw"></p></template>\n';
    const native = compileSfc(src, { filename: 'raw.vue' });
    assert.deepEqual(native.diagnostics.map((d) => d.code), ['VN6110']);
    const webNoTrust = compileSfc(src, { filename: 'raw.vue', target: 'web' });
    assert.deepEqual(webNoTrust.diagnostics.map((d) => d.code), ['VN6110']);
    assert.match(webNoTrust.diagnostics[0].message, /trustedHtml/);
    const webTrust = compileSfc(src, { filename: 'raw.vue', target: 'web', trustedHtml: true });
    assert.deepEqual(webTrust.diagnostics, []);
    assert.equal(typeof webTrust.code, 'string');
  });

  test('web-only attributes and modifiers are accepted on web and rejected on native', () => {
    const src = read('web-attrs.vue');
    const native = compileSfc(src, { filename: 'web-attrs.vue', rootDir: FIXTURES });
    assert.deepEqual(native.diagnostics.map((d) => d.code), ['VN6115', 'VN6115', 'VN6116', 'VN6110']);
    const web = compileSfc(src, { filename: 'web-attrs.vue', rootDir: FIXTURES, target: 'web', trustedHtml: true });
    assert.deepEqual(web.diagnostics, []);
  });

  test('the .passive modifier is rejected on both targets', () => {
    const src = '<template><p @click.passive="go">x</p></template>\n';
    for (const target of ['native', 'web']) {
      assert.deepEqual(compileSfc(src, { filename: 'p.vue', target }).diagnostics.map((d) => d.code), ['VN6111']);
    }
  });
});

describe('paths and files', () => {
  test('a template or style src is read through the project root and its realpath', () => {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), 'vn-sfc-root-'));
    const outside = fs.mkdtempSync(path.join(os.tmpdir(), 'vn-sfc-out-'));
    try {
      fs.writeFileSync(path.join(root, 'inside.html'), '<p>in</p>\n');
      fs.writeFileSync(path.join(outside, 'secret.html'), '<p>secret</p>\n');
      fs.symlinkSync(path.join(outside, 'secret.html'), path.join(root, 'link.html'));
      fs.symlinkSync(path.join(root, 'inside.html'), path.join(root, 'alias.html'));
      const page = (src) => compileSfc(src, { filename: 'Page.vue', rootDir: root });

      const escaped = page('<template src="./link.html"></template>\n');
      assert.deepEqual(escaped.diagnostics.map((d) => d.code), ['VN6008']);
      assert.match(escaped.diagnostics[0].message, /outside the project root/);

      const aliased = page('<template src="./alias.html"></template>\n');
      assert.deepEqual(aliased.diagnostics, [], 'a symlink that stays inside the root is read');
      assert.deepEqual(aliased.dependencies, ['inside.html']);

      const dotdot = page('<template src="../../escape.html"></template>\n');
      assert.deepEqual(dotdot.diagnostics.map((d) => d.code), ['VN6008']);
    } finally {
      fs.rmSync(root, { recursive: true, force: true });
      fs.rmSync(outside, { recursive: true, force: true });
    }
  });

  test('a style src that does not exist is VN6008 at the style tag', () => {
    const r = compileSfc('<template><p/></template>\n<style src="./nope.css"></style>\n', { filename: 'x.vue', rootDir: FIXTURES });
    assert.deepEqual(r.diagnostics.map((d) => `${d.code} ${d.start.line}:${d.start.column}`), ['VN6008 2:0']);
  });
});

describe('registry', () => {
  test('unknown SFC codes are rejected by the lookup and by the diagnostic builder', () => {
    assert.throws(() => sfcCodeInfo('VN6999'), RangeError);
    assert.throws(() => createAnyDiagnostic('VN9998', { file: 'a', start: { offset: 0, line: 1, column: 0 }, end: { offset: 0, line: 1, column: 0 } }), RangeError);
  });

  test('every SFC code has a title and is in the VN6 range', () => {
    for (const code of Object.keys(SFC_CODES)) {
      assert.match(code, /^VN6\d{3}$/);
      assert.ok(SFC_CODES[code].title.length > 0);
    }
  });
});
