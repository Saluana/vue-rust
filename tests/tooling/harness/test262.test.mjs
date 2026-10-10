// WP 3.1: the Test262 tooling. The metadata reader, the selection rules, the one adaptation of the harness, the outcome
// classification and the ledger rule are tested with hand-written inputs; every check has a negative control. The last
// test runs the runner on the negative control file (one debug build of one module).
import { describe, test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { MetaError, parseMeta } from '../../../tooling/harness/test262-meta.mjs';
import { DIRECTORIES, reasonToExclude, sampleEvenly, SUPPORTED_FEATURES } from '../../../tooling/harness/select-test262.mjs';
import { adaptForProfile, classifyCompile, classifyRun, compileModule, ledgerCheck } from '../../../tooling/harness/run-test262.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(here, '..', '..', '..');
const VENDOR = path.join(ROOT, 'tests', 'language', 'test262');
const fm = (body) => `// header comment\n/*---\n${body}\n---*/\n`;

describe('test262-meta: frontmatter reader', () => {
  test('reads flow lists, block lists, a nested negative mapping and a block scalar description', () => {
    const meta = parseMeta(fm([
      'description: >',
      '    a folded description with a colon: inside',
      'includes: [compareArray.js, sta.js]',
      'flags: [onlyStrict]',
      'features:',
      '  - let',
      '  - const',
      'negative:',
      '  phase: runtime',
      '  type: ReferenceError',
    ].join('\n')));
    assert.deepEqual(meta.includes, ['compareArray.js', 'sta.js']);
    assert.deepEqual(meta.flags, ['onlyStrict']);
    assert.deepEqual(meta.features, ['let', 'const']);
    assert.deepEqual(meta.negative, { phase: 'runtime', type: 'ReferenceError' });
  });

  test('a file without the empty-list defaults has no includes, flags, features or negative', () => {
    const meta = parseMeta(fm('description: plain'));
    assert.deepEqual(meta, { description: 'plain', includes: [], flags: [], features: [], negative: null });
  });

  test('negative control: no frontmatter, a tab in the frontmatter and a duplicate key are all refused', () => {
    assert.throws(() => parseMeta('var x = 1;'), MetaError);
    assert.throws(() => parseMeta(fm('description: a\n\tflags: [x]')), MetaError);
    assert.throws(() => parseMeta(fm('flags: [a]\nflags: [b]')), MetaError);
  });
});

describe('test262-select: the selection rules', () => {
  const base = { description: '', includes: [], flags: [], features: [], negative: null };

  test('a plain test with supported metadata is selected', () => {
    assert.equal(reasonToExclude({ ...base, features: ['let', 'arrow-function'], flags: ['onlyStrict'] }), null);
  });

  test('each rule names its reason: flag, feature, include, negative phase and type', () => {
    assert.equal(reasonToExclude({ ...base, flags: ['noStrict'] }), 'flag noStrict');
    assert.equal(reasonToExclude({ ...base, features: ['Proxy'] }), 'feature Proxy');
    assert.equal(reasonToExclude({ ...base, includes: ['propertyHelper.js'] }), 'include propertyHelper.js');
    assert.equal(reasonToExclude({ ...base, negative: { phase: 'resolution', type: 'SyntaxError' } }), 'negative phase resolution');
    assert.equal(reasonToExclude({ ...base, negative: { phase: 'parse', type: 'TypeError' } }), 'negative parse/TypeError');
    assert.equal(reasonToExclude(base, { isFixture: true }), 'module fixture (not a test)');
  });

  test('negative control: the feature list is default-deny (generators and async are not in it)', () => {
    assert.equal(SUPPORTED_FEATURES.has('generators'), false);
    assert.equal(SUPPORTED_FEATURES.has('async-functions'), false);
    assert.equal(reasonToExclude({ ...base, features: ['generators'] }), 'feature generators');
  });

  test('sampling keeps at most the cap, evenly spaced from the first entry; a short list is kept whole', () => {
    const list = Array.from({ length: 25 }, (_, i) => i);
    assert.deepEqual(sampleEvenly(list, 10), [0, 2, 5, 7, 10, 12, 15, 17, 20, 22]);
    assert.deepEqual(sampleEvenly([3, 1, 2], 10), [3, 1, 2]);
  });

  test('the listed directories: 33 of them, none listed twice', () => {
    assert.equal(new Set(DIRECTORIES).size, DIRECTORIES.length);
    assert.equal(DIRECTORIES.length, 33);
  });
});

describe('test262-run: the one adaptation of the harness', () => {
  const sta = fs.readFileSync(path.join(VENDOR, 'harness', 'sta.js'), 'utf8');
  test('the constructor form and the prototype line of sta.js are replaced, and new Test262Error( is rewritten', () => {
    const out = adaptForProfile(sta, 'sta.js');
    assert.match(out, /function Test262Error\(message\) \{\n  return \{ constructor: Test262Error, message: message \|\| "" \};\n\}/);
    assert.doesNotMatch(out, /prototype\.toString/);
    assert.doesNotMatch(out, /new Test262Error\(/);
    assert.equal(adaptForProfile('throw new Test262Error(m);', 'some.js'), 'throw Test262Error(m);');
  });

  test('negative control: a sta.js whose constructor text changed is refused, not adapted silently', () => {
    assert.throws(() => adaptForProfile(sta.replace('this.message', 'this.msg'), 'sta.js'), /no longer has the text/);
  });
});

describe('test262-run: outcome classification', () => {
  const plain = { description: '', includes: [], flags: [], features: [], negative: null };
  const negParse = { ...plain, negative: { phase: 'parse', type: 'SyntaxError' } };
  const negRun = { ...plain, negative: { phase: 'runtime', type: 'ReferenceError' } };
  const ok = (result, error = null) => ({ ok: true, doc: { result, error, entries: [] } });
  const named = (name) => ({ cls: 'Error', name, message: 'm' });

  test('a positive test that compiles and completes passes; one that throws fails', () => {
    assert.equal(classifyRun(plain, ok('ok')).outcome, 'pass');
    assert.equal(classifyRun(plain, ok('throw', { cls: 'Object', props: [] })).outcome, 'fail');
  });

  test('negative runtime: the expected error type passes; another type, or no throw, fails', () => {
    assert.equal(classifyRun(negRun, ok('throw', named('ReferenceError'))).outcome, 'pass');
    assert.equal(classifyRun(negRun, ok('throw', named('TypeError'))).outcome, 'fail');
    assert.equal(classifyRun(negRun, ok('ok')).outcome, 'fail');
  });

  test('a compile diagnostic is rejected-by-profile; a negative parse test passes only on a syntax or early error', () => {
    const diag = (code) => ({ code, severity: 'error', message: 'msg', start: { line: 30 }, end: {} });
    assert.equal(classifyCompile(plain, [diag('VN4017')], 10).outcome, 'rejected-by-profile');
    assert.equal(classifyCompile(negParse, [diag('VN1000')], 10).outcome, 'pass');
    assert.equal(classifyCompile(negParse, [diag('VN5001')], 10).outcome, 'pass');
    // negative control: a profile code is not the expected SyntaxError, and an internal error is never a pass
    assert.equal(classifyCompile(negParse, [diag('VN2002')], 10).outcome, 'rejected-by-profile');
    assert.equal(classifyCompile(negParse, [diag('VN9001')], 10).outcome, 'fail');
    assert.equal(classifyCompile(plain, [], 10), null);
  });

  test('a diagnostic inside the harness is reported as such, and a line in the test is shifted by the harness length', () => {
    // prefixLines 20: line 12 is in the harness, line 24 is line 4 of the test
    const d = { code: 'VN4016', severity: 'error', message: 'global x', start: { line: 12 }, end: {} };
    assert.match(classifyCompile(plain, [d], 20).reason, /harness line 12/);
    const e = { ...d, start: { line: 24 } };
    assert.match(classifyCompile(plain, [e], 20).reason, /\(line 4\)/);
  });

  test('a positive module with valid code compiles, and an unsupported class does not', () => {
    assert.deepEqual(compileModule('let a = 1;\nif (a !== 1) throw new Error("no");', { filename: 'x.js', name: 'x' }).errors, []);
    const bad = compileModule('class C {}', { filename: 'x.js', name: 'x' });
    assert.equal(bad.rust, null);
    assert.ok(bad.errors.some((d) => d.code === 'VN2002'));
  });

  test('the ledger rule: a pass-marked test must pass, a blocked one must not, an unlisted one must pass', () => {
    assert.equal(ledgerCheck('pass', 'pass').ok, true);
    assert.equal(ledgerCheck('pass', 'fail').ok, false, 'a regression');
    assert.equal(ledgerCheck({ blocked: 'x' }, 'rejected-by-profile').ok, true);
    assert.equal(ledgerCheck({ blocked: 'x' }, 'pass').ok, false, 'a ratchet: the test must be flipped to pass');
    assert.equal(ledgerCheck(undefined, 'fail').ok, false);
  });
});

describe('test262-run: the negative control (a copy of a passing test with a wrong expected value)', () => {
  test('the runner reports the control as a fail and exits 1', () => {
    const r = spawnSync(process.execPath, [path.join(ROOT, 'tooling', 'harness', 'run-test262.mjs'), '--dir', path.join(VENDOR, 'controls'), '--no-ledger'], {
      encoding: 'utf8',
      timeout: 600000,
    });
    assert.equal(r.status, 1, r.stdout + r.stderr);
    assert.match(r.stdout, /^fail {16}S15\.8\.2\.1_A1-wrong-expected .*Expected SameValue\(«NaN», «0»\) to be true/m);
    assert.match(r.stdout, /test262 1 files: pass 0, fail 1, rejected-by-profile 0; blocked 0; 1 unexpected/);
  });
});
