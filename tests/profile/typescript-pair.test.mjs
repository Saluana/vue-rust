// The TypeScript 6 + Vue language-tools pair (task 2.2): exact pins, installed versions, the manifest entries, the
// TypeScript 7 exclusion, the probe run on its fixture, and negative controls for a different installed version.
// Expected versions are hand-written from the maintainer's pins, not read back from the code under test.
import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { lookup, isSupported } from '../../compiler/profile/index.mjs';
import { checkToolingPair, installedVersions, TOOLING_PAIR } from '../../compiler/types/pair.mjs';
import { fakeCheckout } from '../scratch.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const PROBE = path.join(ROOT, 'tooling', 'types', 'probe.mjs');
const PROBE_PROJECT = path.join(ROOT, 'tests', 'compiler', 'types', 'fixtures', 'pair');
const readJson = (file) => JSON.parse(fs.readFileSync(file, 'utf8'));

const PINNED_DEV = { typescript: '6.0.3', vue: '3.5.43', 'vue-tsc': '3.3.12' };
const PINNED_INSTALLED = { typescript: '6.0.3', vue: '3.5.43', 'vue-tsc': '3.3.12', '@vue/language-core': '3.3.12', '@volar/typescript': '2.4.28' };

describe('the pins', () => {
  test('package.json pins TypeScript and vue-tsc exactly (no range)', () => {
    const dev = readJson(path.join(ROOT, 'package.json')).devDependencies;
    assert.equal(dev.typescript, '6.0.3');
    assert.equal(dev['vue-tsc'], '3.3.12');
  });

  test('the pair constants in compiler/types/pair.mjs are the hand-written pins', () => {
    assert.deepEqual({ ...TOOLING_PAIR }, {
      typescript: '6.0.3', 'vue-tsc': '3.3.12', '@vue/language-core': '3.3.12', '@volar/typescript': '2.4.28',
    });
  });

  test('vue-tsc declares exact versions for the language core and the volar binding it runs with', () => {
    const declared = readJson(path.join(ROOT, 'node_modules', 'vue-tsc', 'package.json')).dependencies;
    assert.equal(declared['@vue/language-core'], '3.3.12');
    assert.equal(declared['@volar/typescript'], '2.4.28');
  });
});

describe('installed versions', () => {
  test('node_modules holds the pinned pair (read directly from each package.json)', () => {
    const installed = {
      typescript: readJson(path.join(ROOT, 'node_modules', 'typescript', 'package.json')).version,
      'vue-tsc': readJson(path.join(ROOT, 'node_modules', 'vue-tsc', 'package.json')).version,
      '@vue/language-core': readJson(path.join(ROOT, 'node_modules', '@vue', 'language-core', 'package.json')).version,
      '@volar/typescript': readJson(path.join(ROOT, 'node_modules', '@volar', 'typescript', 'package.json')).version,
      vue: readJson(path.join(ROOT, 'node_modules', 'vue', 'package.json')).version,
    };
    assert.deepEqual(installed, PINNED_INSTALLED);
  });

  test('the resolver used at run time reads the same versions', () => {
    const versions = installedVersions(ROOT);
    assert.deepEqual(versions, PINNED_INSTALLED);
  });

  test('the checkout passes the pair check with no problems', () => {
    const check = checkToolingPair(ROOT);
    assert.equal(check.ok, true, check.problems.join('; '));
    assert.deepEqual(check.problems, []);
  });
});

describe('the manifest records the pair and the TypeScript 7 exclusion', () => {
  test('TypeScript 6.0.3 and vue-tsc 3.3.12 are supported adaptation entries of row S23', () => {
    assert.deepEqual(lookup('config', 'typescript@6.0.3'), {
      status: 'A', row: 'S23', note: lookup('config', 'typescript@6.0.3').note, compilerInternal: false,
    });
    assert.equal(lookup('config', 'vue-tsc@3.3.12').status, 'A');
    assert.equal(lookup('config', 'vue-tsc@3.3.12').row, 'S23');
    assert.equal(isSupported('config', 'typescript@6.0.3', 'native'), true);
    assert.equal(isSupported('config', 'vue-tsc@3.3.12', 'web'), true);
  });

  test('TypeScript 7 is excluded (X) with the design lines that require it', () => {
    const entry = lookup('config', 'typescript@7');
    assert.equal(entry.status, 'X');
    assert.equal(entry.row, 'S23');
    assert.match(entry.note, /design\.md line 573/);
    assert.match(entry.note, /K07 at line 734/);
    assert.match(entry.note, /JavaScript compiler API/);
    assert.equal(isSupported('config', 'typescript@7', 'native'), false);
    assert.equal(isSupported('config', 'typescript@7', 'web'), false);
  });

  test('the design lines cited by the TypeScript 7 entry say what the note claims', () => {
    const design = fs.readFileSync(path.join(ROOT, 'planning', 'vue-native-framework', 'design.md'), 'utf8').split('\n');
    assert.match(design[572], /TypeScript 7's API gap is a gate; pin a proven TypeScript 6 integration/);
    assert.match(design.find((line) => line.startsWith('| K07 |')), /TypeScript 7 API absence complicates Vue tooling; exact 6\.x pair unknown/);
  });
});

describe('negative controls: a different installed version is refused', () => {
  test('TypeScript 7 in node_modules fails the pair check (no JavaScript compiler API for the Vue language tools)', () => {
    const dir = fakeCheckout('ts7', {
      devDependencies: PINNED_DEV,
      installed: { ...PINNED_INSTALLED, typescript: '7.0.2' },
    });
    const check = checkToolingPair(dir);
    assert.equal(check.ok, false);
    assert.ok(check.problems.some((p) => /TypeScript 7\.0\.2 is installed/.test(p)), check.problems.join('; '));
  });

  test('a different vue-tsc version fails the pair check', () => {
    const dir = fakeCheckout('vuetsc', {
      devDependencies: PINNED_DEV,
      installed: { ...PINNED_INSTALLED, 'vue-tsc': '3.3.11' },
    });
    const check = checkToolingPair(dir);
    assert.equal(check.ok, false);
    assert.deepEqual(check.problems, ['vue-tsc 3.3.11, pinned 3.3.12']);
  });

  test('a different language core under vue-tsc fails the pair check', () => {
    const dir = fakeCheckout('core', {
      devDependencies: PINNED_DEV,
      installed: { ...PINNED_INSTALLED, '@vue/language-core': '3.3.13' },
    });
    assert.deepEqual(checkToolingPair(dir).problems, ['@vue/language-core 3.3.13, pinned 3.3.12']);
  });

  test('a range instead of an exact pin in package.json fails the pair check', () => {
    const dir = fakeCheckout('range', {
      devDependencies: { ...PINNED_DEV, typescript: '^6.0.3' },
      installed: PINNED_INSTALLED,
    });
    assert.deepEqual(checkToolingPair(dir).problems, ['package.json declares typescript ^6.0.3, expected the exact pin 6.0.3']);
  });

  test('a checkout without node_modules is reported, not guessed', () => {
    const dir = fakeCheckout('empty', { devDependencies: PINNED_DEV, installed: {} });
    const check = checkToolingPair(dir);
    assert.equal(check.ok, false);
    assert.ok(check.problems.includes('typescript is not installed, pinned 6.0.3'), check.problems.join('; '));
  });
});

describe('the probe', () => {
  test('runs the pair on its fixture: pinned versions and the .vue line and column of two known errors', () => {
    const run = spawnSync(process.execPath, [PROBE, PROBE_PROJECT, '--json'], { encoding: 'utf8' });
    assert.equal(run.status, 1, run.stderr);
    const report = JSON.parse(run.stdout);
    assert.deepEqual(report.versions, PINNED_INSTALLED);
    assert.deepEqual(report.modules, ['/src/App.vue', '/src/Child.vue']);
    // Hand-counted from fixtures/pair/src/App.vue: line 6 starts `count.value`; line 11 is `  <Child :label="3" />`, where
    // `label` starts at column 10 (0-based). Columns are 0-based, as in every diagnostic.
    assert.deepEqual(report.diagnostics.map(({ file, line, column, severity, code }) => ({ file, line, column, severity, code })), [
      { file: 'src/App.vue', line: 6, column: 0, severity: 'error', code: 'VN7001' },
      { file: 'src/App.vue', line: 11, column: 10, severity: 'error', code: 'VN7001' },
    ]);
    assert.match(report.diagnostics[0].message, /^TS2322: Type 'string' is not assignable to type 'number'\.$/);
    assert.match(report.diagnostics[1].message, /^TS2322: Type 'number' is not assignable to type 'string'\.$/);
  });

  test('a usage error exits 2 and names the problem', () => {
    const run = spawnSync(process.execPath, [PROBE, path.join(ROOT, 'no-such-project')], { encoding: 'utf8' });
    assert.equal(run.status, 2);
    assert.match(run.stderr, /no such project directory/);
  });
});
