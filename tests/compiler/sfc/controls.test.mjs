// Negative controls (WP-F1). They show that the checks in the other files can fail: a span off by one is not accepted,
// and a fixture broken on purpose produces a different diagnostic or an error.
import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { compileSfc } from '../../../compiler/sfc/index.mjs';
import { FIXTURES } from './harness.mjs';

const spanOf = (d) => `${d.start.line}:${d.start.column}-${d.end.line}:${d.end.column}`;
const read = (f) => fs.readFileSync(path.join(FIXTURES, f), 'utf8');

describe('negative controls', () => {
  test('a span that is off by one column is not accepted', () => {
    const r = compileSfc(read('bad-tag-table.vue'), { filename: 'bad-tag-table.vue', rootDir: FIXTURES });
    const [d] = r.diagnostics;
    assert.equal(d.code, 'VN6102');
    assert.equal(spanOf(d), '2:3-2:8', 'the hand-counted span');
    assert.notEqual(spanOf(d), '2:4-2:8', 'a start one column late is a mismatch');
    assert.notEqual(spanOf(d), '2:2-2:8', 'a start one column early is a mismatch');
    assert.notEqual(spanOf(d), '2:3-2:9', 'an end one column late is a mismatch');
  });

  test('a span that is off by one line is not accepted', () => {
    const r = compileSfc(read('bad-tag-table.vue'), { filename: 'bad-tag-table.vue', rootDir: FIXTURES });
    assert.notEqual(spanOf(r.diagnostics[0]), '1:3-1:8');
  });

  test('a fixture broken on purpose gets a different code', () => {
    const broken = read('bad-tag-table.vue').replace('<table>', '<tabl>').replace('</table>', '</tabl>');
    const r = compileSfc(broken, { filename: 'bad-tag-table.vue', rootDir: FIXTURES });
    assert.deepEqual(r.diagnostics.map((d) => d.code), ['VN6101'], 'an unknown tag is VN6101, not the excluded-tag code');
  });

  test('a clean fixture with one unsupported global gets exactly that diagnostic', () => {
    const broken = read('ok-counter.vue').replace('<template>', '<template>\n  <p>{{ Proxy }}</p>');
    const r = compileSfc(broken, { filename: 'ok-counter.vue', rootDir: FIXTURES });
    assert.deepEqual(r.diagnostics.map((d) => d.code), ['VN4003']);
    assert.equal(r.code, null);
  });

  test('removing the unsupported import from a rejecting fixture makes it accept', () => {
    const fixed = read('bad-vue-import-h.vue').replace("import { h } from 'vue'\n", '');
    const r = compileSfc(fixed, { filename: 'bad-vue-import-h.vue', rootDir: FIXTURES });
    assert.deepEqual(r.diagnostics, [], 'the fix removes the only diagnostic');
    assert.equal(typeof r.code, 'string');
  });
});

test('slot aliases do not expose parameters of nested default functions to the template', () => {
  const prefix = '<script setup>const Child = {}</script><template><Child v-slot="{ item = (inner) => inner }">';
  const valid = compileSfc(`${prefix}{{ item }}</Child></template>`, { filename: 'scope.vue' });
  assert.deepEqual(valid.diagnostics, []);
  const invalid = compileSfc(`${prefix}{{ inner }}</Child></template>`, { filename: 'scope.vue' });
  assert.equal(invalid.code, null);
  assert.ok(invalid.diagnostics.some(d => d.code === 'VN4016' && d.message.includes('inner')));
});
