// HIR schema hash and shape (task 4.1). The catalogue in compiler/hir/schema.mjs must describe the HIR that the lowering
// produces: every module of tests/language/lowering, and two inline modules that cover what those fixtures do not
// (imports, re-exports, `delete` of a call, empty branches, a non-declaration for-of head), lowers to a record whose nodes
// and records are all listed, and every catalogue entry is produced by some module. The committed hash below is the
// sha256 of the canonical JSON of the catalogue at hirVersion 1; it was checked against Python's hashlib and sha256sum
// over the same text. A catalogue change must come with a new hirVersion and a new entry in COMMITTED.
import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkSource } from '../../../compiler/frontend/check.mjs';
import { lowerToHir, HIR_VERSION } from '../../../compiler/hir/index.mjs';
import {
  HIR_NODES, HIR_RECORDS, HIR_SCHEMA_HASH, canonicalJson, catalogueEntryIds, checkHirShape, hirSchemaHash,
} from '../../../compiler/hir/schema.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '../../..');
const LOWERING = path.join(repo, 'tests/language/lowering');

const COMMITTED = Object.freeze({
  1: '7e6bf7b654d1153cb813afa35fe94e26053d7b3393bb009e6f272c496e8a0e68',
  // 2: await/throw nodes and functions carry a source site, modules list them (task 3.7).
  2: '9d7cbb3f1dcbb6b36009a7756e82810a92a883867938d00b2fac20e86f0a1c70',
});

// Modules the lowering fixtures do not cover. Each is checked with checkSource and lowered like a fixture (target test).
const SAMPLES = {
  'sample/module-records': [
    "import def, { named as renamed } from './dep.js'",
    "import * as ns from './ns.js'",
    "export { renamed as alias } from './dep.js'",
    "export * as star from './star.js'",
    "export * from './all.js'",
    'export const kept = def + ns.x',
    '',
  ].join('\n'),
  'sample/statement-forms': [
    'let t = {}; let u;',
    'function g() { return 1 }',
    'delete g();',
    'if (t) ; else ;',
    'while (u) ;',
    'let w; for (w of [1]) ;',
    '',
  ].join('\n'),
};

function lowerSource(name, source) {
  const { ast, analysis, diagnostics } = checkSource(source, { filename: name, lang: 'js', target: 'test' });
  assert.ok(ast && !diagnostics.some((d) => d.severity === 'error'), `${name} passes the frontend`);
  const { hir, diagnostics: lowering } = lowerToHir(ast, analysis, { source, filename: name, specifier: name, lang: 'js' });
  assert.ok(hir !== null, `${name} lowers: ${lowering.map((d) => d.code + ' ' + d.message).join('; ')}`);
  return hir;
}

// Every module of the walk: [name, source] in name order (the fixtures, then the samples).
function modules() {
  const files = fs.readdirSync(LOWERING).filter((f) => f.endsWith('.js')).sort();
  const out = files.map((f) => [f, fs.readFileSync(path.join(LOWERING, f), 'utf8')]);
  return out.concat(Object.entries(SAMPLES));
}

const LOWERED = modules().map(([name, source]) => ({ name, hir: lowerSource(name, source) }));

describe('HIR schema hash', () => {
  test('HIR_SCHEMA_HASH recomputed from the catalogue matches the hash committed for HIR_VERSION', () => {
    assert.equal(HIR_VERSION, 2, 'a new hirVersion needs its own committed hash (see the header)');
    assert.equal(hirSchemaHash(), HIR_SCHEMA_HASH);
    assert.equal(HIR_SCHEMA_HASH, COMMITTED[HIR_VERSION]);
  });

  test('the hash is over the canonical JSON: key order in the source does not change it', () => {
    const reordered = { records: HIR_RECORDS, nodes: HIR_NODES };
    assert.equal(canonicalJson(reordered), canonicalJson({ nodes: HIR_NODES, records: HIR_RECORDS }));
    assert.equal(hirSchemaHash(reordered), COMMITTED[HIR_VERSION]);
  });

  test('negative control: a mutated catalogue changes the hash', () => {
    const cases = {
      'a field removed from var': { nodes: { ...HIR_NODES, var: [['pos', 'r', 'name']] }, records: HIR_RECORDS },
      'a field added to num': { nodes: { ...HIR_NODES, num: [['pos', 's', 'raw']] }, records: HIR_RECORDS },
      'a record field renamed': {
        nodes: HIR_NODES,
        records: { ...HIR_RECORDS, Fn: [HIR_RECORDS.Fn[0].map((f) => (f === 'caps' ? 'captures' : f))] },
      },
      'a node kind dropped': { nodes: Object.fromEntries(Object.entries(HIR_NODES).filter(([k]) => k !== 'hole')), records: HIR_RECORDS },
    };
    for (const [label, mutated] of Object.entries(cases)) {
      assert.notEqual(hirSchemaHash(mutated), COMMITTED[HIR_VERSION], `${label} must change the hash`);
    }
  });
});

describe('HIR shape against the producer', () => {
  test('every node and record of every walked module is listed in the catalogue', () => {
    const problems = [];
    for (const { name, hir } of LOWERED) {
      for (const v of checkHirShape(hir).violations) problems.push(`${name} ${v.path}: ${v.message}`);
    }
    assert.deepEqual(problems, []);
    assert.ok(LOWERED.length >= 166 + 2, `walked ${LOWERED.length} modules`);
  });

  test('every catalogue entry is produced by at least one walked module (no dead entries)', () => {
    const met = new Set();
    for (const { hir } of LOWERED) for (const id of checkHirShape(hir).met) met.add(id);
    const unmet = catalogueEntryIds().filter((id) => !met.has(id));
    assert.deepEqual(unmet, []);
  });

  test('the module record carries the hirVersion the catalogue is for', () => {
    for (const { hir } of LOWERED) assert.equal(hir.hirVersion, HIR_VERSION, hir.specifier);
  });

  test('negative control: an unknown node kind is reported with its path', () => {
    const hir = lowerSource('neg/unknown-kind.js', 'let n = 1\ntrace(n)\n');
    hir.body.push({ k: 'bogus', pos: [0, 0] });
    const { violations } = checkHirShape(hir);
    assert.equal(violations.length, 1);
    assert.equal(violations[0].path, '$.body[2]');
    assert.match(violations[0].message, /unknown node kind "bogus"/);
  });

  test('negative control: a node field the catalogue does not list, and a missing record field, are reported', () => {
    const hir = lowerSource('neg/fields.js', 'let n = 1\ntrace(n)\n');
    const num = hir.body[0].v;
    assert.equal(num.k, 'num');
    num.extra = true;
    const messages = checkHirShape(hir).violations.map((v) => v.message);
    assert.ok(messages.some((m) => /node "num" has fields \[extra, pos, s\]/.test(m)), messages.join(' | '));

    const plain = lowerSource('neg/fields-fn.js', 'function f(a) { return a }\ntrace(f(1))\n');
    const rec = plain.fns.find((f) => f && f.name === 'f');
    delete rec.length;
    const records = checkHirShape(plain).violations.map((v) => v.message);
    assert.ok(records.some((m) => /record with fields \[.*\] matches no catalogue record/.test(m)), records.join(' | '));
  });

  test('negative control: a catalogue entry that nothing produces is reported as unmet', () => {
    const hir = lowerSource('neg/unmet.js', 'let n = 1\ntrace(n)\n');
    const extended = { nodes: { ...HIR_NODES, num: [...HIR_NODES.num, ['pos', 's', 'raw']] }, records: HIR_RECORDS };
    const met = checkHirShape(hir, extended).met;
    assert.ok(met.has('node:num#0'));
    assert.ok(!met.has('node:num#1'), 'the extra variant is not met');
    assert.ok(catalogueEntryIds(extended).includes('node:num#1'));
  });
});
