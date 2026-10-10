// Source positions of generated code (WP 3.7): the VLQ decoder of source maps, the locators that map compiled offsets to the
// author's file, and the SITES table that the emitter writes for throw, await and function entries. Expected positions are
// hand-derived from the fixture text (tests/cli/fixtures/throw-async/src/App.vue), not read from the code under test.
import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { compileSfc } from '../../compiler/sfc/index.mjs';
import { decodeMappings, identityLocator, sourceMapLocator } from '../../compiler/sourcemap.mjs';
import { compileAt } from '../../tooling/cli/project.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const FIXTURE = path.join(ROOT, 'tests', 'cli', 'fixtures', 'throw-async');
// Hand-derived from the fixture text (1-based lines, 0-based columns):
//   line 6  `async function load() {`         column 0 is `a` of async
//   line 8  `  await Promise.resolve()`        the await is at 8:2
//   line 9  `  throw new Error('load failed')` the throw is at 9:2
const EXPECTED = { await: { line: 8, column: 2 }, throw: { line: 9, column: 2 }, load: { line: 6, column: 0 } };

describe('decodeMappings: the v3 VLQ mappings of a source map', () => {
  test('segments of two lines, with source fields relative to the previous segment', () => {
    // "AAAA" = [0, 0, 0, 0]; line 2 "AACA" = [0, 0, 1, 0] (source line + 1); "EAAE" = [2, 0, 0, 2] (generated column + 2
    // from the line start, source column + 2 from the previous segment's source column 0)
    assert.deepEqual(decodeMappings('AAAA;AACA,EAAE'), [[[0, 0, 0, 0]], [[0, 0, 1, 0], [2, 0, 1, 2]]]);
  });

  test('negative control: a character outside the base64 alphabet is refused', () => {
    assert.throws(() => decodeMappings('AA!A'), /bad character/);
  });
});

describe('sourceMapLocator: positions of a compiled .vue module', () => {
  const source = fs.readFileSync(path.join(FIXTURE, 'src', 'App.vue'), 'utf8');
  const sfc = compileSfc(source, { filename: 'App.vue', rootDir: path.join(FIXTURE, 'src'), target: 'native' });
  assert.equal(sfc.diagnostics.length, 0, JSON.stringify(sfc.diagnostics.map((d) => d.message)));
  const locate = sourceMapLocator({ code: sfc.code, map: sfc.map, file: 'src/App.vue', mapOffset: sfc.mapOffset });
  const at = (text) => {
    const offset = sfc.code.indexOf(text);
    assert.ok(offset >= 0, `${text} is in the compiled code`);
    return locate(offset);
  };

  test('the await, the throw and the function entry map to their .vue lines and columns', () => {
    assert.deepEqual(at('await Promise.resolve()'), { file: 'src/App.vue', ...EXPECTED.await });
    assert.deepEqual(at("throw new Error('load failed')"), { file: 'src/App.vue', ...EXPECTED.throw });
    assert.deepEqual(at('async function load()'), { file: 'src/App.vue', ...EXPECTED.load });
  });

  test('negative control: the throw moved by two lines is reported two lines lower, not at the old line', () => {
    const moved = source.replace("  throw new Error('load failed')", "\n\n  throw new Error('load failed')");
    const again = compileSfc(moved, { filename: 'App.vue', rootDir: path.join(FIXTURE, 'src'), target: 'native' });
    const locateMoved = sourceMapLocator({ code: again.code, map: again.map, file: 'src/App.vue', mapOffset: again.mapOffset });
    const got = locateMoved(again.code.indexOf("throw new Error('load failed')"));
    assert.deepEqual(got, { file: 'src/App.vue', line: EXPECTED.throw.line + 2, column: EXPECTED.throw.column });
    assert.notDeepEqual(got, { file: 'src/App.vue', ...EXPECTED.throw });
  });

  test("the header of the compiled text, which is not the author's, has no position", () => {
    assert.equal(locate(0), null);
  });
});

describe('identityLocator: code that is its own source', () => {
  test('offsets are positions in the file, 1-based lines and 0-based columns', () => {
    const locate = identityLocator('a\n  bc\n', 'src/x.js');
    assert.deepEqual(locate(0), { file: 'src/x.js', line: 1, column: 0 });
    assert.deepEqual(locate(5), { file: 'src/x.js', line: 2, column: 3 });
  });
});

describe('SITES: the emitted source sites of a compiled module', () => {
  test('throw and await calls carry the sites of their statements, and functions carry their entries', () => {
    const c = compileAt({ root: FIXTURE, entry: 'src/App.vue', target: 'native' });
    assert.equal(c.ok, true, JSON.stringify(c.diagnostics.map((d) => d.message)));
    const vue = c.modules.find((m) => m.kind === 'vue');
    const start = vue.rust.indexOf('pub static SITES');
    const sites = vue.rust.slice(start, vue.rust.indexOf('];', start) + 2);
    assert.match(sites, new RegExp(`SiteKind::Await, name: "", file: "src/App.vue", line: ${EXPECTED.await.line}, column: ${EXPECTED.await.column} }`));
    assert.match(sites, new RegExp(`SiteKind::Throw, name: "", file: "src/App.vue", line: ${EXPECTED.throw.line}, column: ${EXPECTED.throw.column} }`));
    assert.match(sites, new RegExp(`SiteKind::Function, name: "load", file: "src/App.vue", line: ${EXPECTED.load.line}, column: ${EXPECTED.load.column} }`));
    // the throw in load names load's entry and the await the function had resumed from
    assert.match(vue.rust, /rt\.throw_at\(SITES, \d+, \d+, vn_await_site\(&frame_get\(rt, fr, \d+\)\), /);
    assert.match(vue.rust, /frame_set\(rt, fr, \d+, Value::Number\(\d+\.0\)\);\n\s*frame_set\(rt, fr, \d+, await_value\(/);
  });

  test('negative control: a synchronous function gets no await site on its throws', () => {
    const c = compileAt({ root: path.join(ROOT, 'tests', 'cli', 'fixtures', 'throw-setup'), entry: 'src/App.vue', target: 'native' });
    assert.equal(c.ok, true, JSON.stringify(c.diagnostics.map((d) => d.message)));
    const vue = c.modules.find((m) => m.kind === 'vue');
    assert.match(vue.rust, /rt\.throw_at\(SITES, \d+, \d+, NO_SITE, /);
    assert.doesNotMatch(vue.rust, /vn_await_site/);
  });
});
