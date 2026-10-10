// Diagnostics fixtures (WP-F1). Each row names a rejecting fixture with the exact code and the span the diagnostic must
// have, written as line:column-line:column (1-based lines, 0-based UTF-16 columns, end exclusive). The spans were counted
// by hand from the fixture text; they are not read from the compiler. A fixture that compiles must produce no error.
import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { compileSfc, serializeDiagnostics, SFC_CODES } from '../../../compiler/sfc/index.mjs';
import { FIXTURES } from './harness.mjs';

const spanOf = (d) => `${d.start.line}:${d.start.column}-${d.end.line}:${d.end.column}`;

// [fixture, [[code, span], ...]] (every row lists all diagnostics of that fixture, in order).
const EXPECTED = [
  ['bad-custom-block.vue', [['VN6003', '5:0-5:6']]],
  ['bad-style-scss.vue', [['VN6004', '5:0-5:19']]],
  ['bad-template-pug.vue', [['VN6004', '1:0-1:21']]],
  ['bad-script-src.vue', [['VN6005', '1:0-1:31']]],
  ['bad-script-tsx.vue', [['VN6006', '1:0-1:19']]],
  ['bad-options-data.vue', [['VN6007', '3:2-5:3']]],
  ['bad-options-methods.vue', [['VN6007', '3:2-3:22']]],
  // defineComponent({ name }) is accepted since A10 (decision 14), so this fixture passes an argument that cannot be read statically.
  ['bad-default-export-call.vue', [['VN6007', '4:0-4:39']]],
  ['bad-define-component-render.vue', [['VN6007', '5:2-7:3']]], // the Options render option stays X (A18)
  ['bad-define-component-async.vue', [['VN6007', '5:2-7:3']]], // async setup needs Suspense, which is experimental (S14)
  ['bad-define-component-extra.vue', [['VN6007', '4:0-4:74']]], // the function form takes one argument in an SFC default export
  // A template beside a normal <script> reads only <script setup> names, so setup state is not visible to it (decision 14).
  ['bad-define-component-template-state.vue', [['VN4016', '10:16-10:17']]],
  ['bad-name-not-string.vue', [['VN6007', '3:2-3:9']]],
  ['bad-setup-export.vue', [['VN6009', '2:0-2:18']]],
  ['bad-dup-template.vue', [['VN6002', '2:0-2:29']]],
  ['bad-dup-setup.vue', [['VN6002', '4:0-6:9']]],
  ['bad-template-src-outside.vue', [['VN6008', '1:0-1:32']]],
  ['bad-style-src-missing.vue', [['VN6008', '1:0-1:27']]],
  ['bad-style-module.vue', [['VN6015', '2:0-2:14']]],
  ['bad-only-style.vue', [['VN6010', '1:0-1:7']]], // Vue's zero-width location, widened to the <style> tag
  ['bad-reserved-sfc.vue', [['VN6011', '2:6-2:11']]],
  ['bad-reserved-render.vue', [['VN6011', '3:9-3:15']]],
  ['bad-parse-unclosed.vue', [['VN6001', '2:0-2:3']]],
  ['bad-expr-syntax.vue', [['VN1000', '5:8-5:8']]],
  ['bad-define-twice.vue', [['VN6012', '1:0-1:14']]],
  // A type-only import of a file that is not in the project: the span is the import source (S19). Vue alone would type the
  // prop as unknown without an error. The outside file exists beside the fixtures root and is refused all the same.
  ['bad-import-type-missing.vue', [['VN6017', '2:29-2:49']]],
  ['bad-import-type-outside.vue', [['VN6017', '2:29-2:44']]],
  ['bad-v-else.vue', [['VN6013', '2:2-2:17']]],
  ['bad-css-syntax.vue', [['VN6014', '2:0-2:1']]],
  ['bad-tag-mark.vue', [['VN6101', '2:3-2:7']]],
  ['bad-tag-component.vue', [['VN6101', '2:3-2:6']]],
  ['bad-custom-element.vue', [['VN6101', '2:3-2:12']]],
  ['bad-tag-table.vue', [['VN6102', '2:3-2:8']]],
  ['bad-tag-video.vue', [['VN6102', '2:3-2:8']]],
  ['bad-tag-suspense.vue', [['VN6103', '2:3-2:11']]],
  ['bad-tag-base-transition.vue', [['VN6104', '2:3-2:17']]],
  ['bad-v-html.vue', [['VN6110', '2:5-2:17']]],
  ['bad-passive.vue', [['VN6111', '2:10-2:29']]],
  ['bad-input-file.vue', [['VN6112', '2:9-2:20']]],
  ['bad-input-date.vue', [['VN6112', '2:9-2:20']]],
  ['bad-input-number.vue', [['VN6112', '2:9-2:22']]],
  ['bad-input-dynamic.vue', [['VN6113', '2:9-2:21']]],
  ['bad-directive-unbound.vue', [['VN6114', '2:5-2:12']]],
  ['bad-modifier-camel.vue', [['VN6115', '2:5-2:21']]],
  ['bad-modifier-unknown.vue', [['VN6115', '2:10-2:27']]],
  ['bad-attr-contenteditable.vue', [['VN6116', '2:5-2:27']]],
  ['bad-instance-el.vue', [['VN6117', '2:8-2:11']]],
  ['bad-vue-import-create-renderer.vue', [['VN6201', '2:9-2:23']]],
  ['bad-vue-default-import.vue', [['VN6202', '2:7-2:10']]],
  ['bad-vue-namespace.vue', [['VN6202', '2:7-2:13']]],
  ['bad-vue-subpath.vue', [['VN6203', '2:0-2:52']]],
  ['bad-node-builtin.vue', [['VN6204', '2:15-2:24']]],
  ['bad-proxy-astral.vue', [['VN4003', '2:11-2:16']]],
  ['bad-eval-setup.vue', [['VN4001', '2:0-2:4']]],
  ['bad-require.vue', [['VN4009', '2:10-2:17']]],
  ['bad-window-native.vue', [['VN4011', '2:10-2:16']]],
  ['bad-class.vue', [['VN2002', '2:0-2:7']]],
  ['bad-with.vue', [['VN2001', '2:0-2:4']]],
  ['bad-generator.vue', [['VN2008', '2:0-2:15'], ['VN2009', '2:18-2:23']]],
  ['bad-await-toplevel.vue', [['VN2022', '2:0-2:5']]],
  ['bad-regexp.vue', [['VN2011', '2:10-2:13']]],
  ['bad-proto-access.vue', [['VN2017', '2:10-2:19']]],
  ['bad-jsx-setup.vue', [['VN1000', '2:11-2:12']]],
  ['bad-ts-enum.vue', [['VN3001', '2:0-2:10']]],
  ['bad-ts-namespace.vue', [['VN3002', '2:0-2:11']]],
  ['bad-ts-using.vue', [['VN3008', '2:0-2:5']]],
  ['bad-let-dup.vue', [['VN5001', '3:4-3:5']]],
  ['bad-const-assign.vue', [['VN5002', '3:0-3:1']]], // a warning: the component still compiles
  ['bad-v-model-reactive-const.vue', [['VN5002', '8:30-8:34']]],
  ['bad-empty-dynamic-arg.vue', [['VN1000', '2:6-2:8']]],
  ['bad-empty-interpolation.vue', [['VN1000', '2:8-2:8']]],
  ['bad-macro-import.vue', [['VN6016', '2:9-2:20']]], // a warning: the macro import is redundant
];

function compileFixture(file) {
  const src = fs.readFileSync(path.join(FIXTURES, file), 'utf8');
  return { src, result: compileSfc(src, { filename: file, rootDir: FIXTURES }) };
}

describe('diagnostics fixtures', () => {
  test('the table covers every rejecting fixture and every fixture file is listed', () => {
    const onDisk = fs.readdirSync(FIXTURES).filter((f) => f.startsWith('bad-') && f.endsWith('.vue')).sort();
    assert.deepEqual(EXPECTED.map(([f]) => f).sort(), onDisk);
    assert.ok(EXPECTED.length >= 40, `at least 40 diagnostics fixtures (have ${EXPECTED.length})`);
  });

  for (const [file, expected] of EXPECTED) {
    test(`${file}: exact code and span`, () => {
      const { result } = compileFixture(file);
      const got = result.diagnostics.map((d) => `${d.code} ${spanOf(d)}`);
      assert.deepEqual(got, expected.map(([code, span]) => `${code} ${span}`));
      for (const d of result.diagnostics) assert.equal(d.file, file, 'diagnostic names the .vue file');
      const errors = result.diagnostics.filter((d) => d.severity === 'error');
      if (errors.length) {
        assert.equal(result.code, null, 'no code is emitted when an error exists');
        assert.deepEqual(result.styles, []);
      }
    });
  }

  test('a warning does not stop compilation', () => {
    const { result } = compileFixture('bad-const-assign.vue');
    assert.equal(result.diagnostics[0].severity, 'warning');
    assert.equal(typeof result.code, 'string');
  });

  test('every error registered in SFC_CODES is produced by a fixture row', () => {
    const produced = new Set(EXPECTED.flatMap(([, rows]) => rows.map(([code]) => code)));
    const unused = Object.keys(SFC_CODES).filter((code) => !produced.has(code));
    assert.deepEqual(unused, [], 'codes without a fixture');
  });

  test('registry entries have a title, an explanation and a hint', () => {
    for (const [code, info] of Object.entries(SFC_CODES)) {
      for (const field of ['title', 'explanation', 'hint']) {
        assert.ok(typeof info[field] === 'string' && info[field].length > 0, `${code}.${field}`);
      }
    }
  });

  test('serializeDiagnostics prints the caret under the offending span', () => {
    const { src, result } = compileFixture('bad-tag-table.vue');
    const text = serializeDiagnostics(result.diagnostics, src, { sources: result.sources });
    const lines = text.split('\n');
    assert.equal(lines[0], "bad-tag-table.vue:2:3: error VN6102: '<table>' is excluded from the native profile");
    assert.equal(lines[1], '  |');
    assert.equal(lines[2], '2 |   <table></table>');
    assert.equal(lines[3], '  |    ^^^^^');
  });

  test('astral characters count as two UTF-16 units before the span', () => {
    const { src, result } = compileFixture('bad-proxy-astral.vue');
    const [d] = result.diagnostics;
    assert.equal(src.slice(d.start.offset, d.end.offset), 'Proxy');
    assert.equal(d.start.column, 11, 'the emoji occupies two columns');
  });
});
