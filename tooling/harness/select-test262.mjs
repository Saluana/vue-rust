#!/usr/bin/env node
// Selects the Test262 files that the language profile can run (WP 3.1) and vendors them under tests/language/test262/.
//   node tooling/harness/select-test262.mjs --upstream <test262 checkout> [--write | --check]
// The rules are the code below, applied to the metadata of every file in the listed directories (no hand picking).
// Step 1: a file is eligible when it is a test (not a *_FIXTURE.js module), its metadata is readable and every rule of
// `reasonToExclude` passes; the first failing rule is the recorded reason. Step 2: at most MAX_PER_DIRECTORY eligible files
// per listed directory are kept by systematic sampling over the sorted paths (evenly spaced, so no run-to-run choice).
// --write copies the selected files, the harness files they include (assert.js and sta.js always, compareArray.js only when
// a selected test includes it), the LICENSE and a README stating the upstream commit and these rules. --check verifies that
// the vendored tree is exactly what these rules select from the checkout.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { MetaError, parseMeta } from './test262-meta.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
export const ROOT = path.resolve(here, '../..');
export const VENDOR_DIR = path.join(ROOT, 'tests', 'language', 'test262');

// Directories under test/. Each is walked as a directory (a sibling such as for-in is never picked by name prefix).
export const DIRECTORIES = Object.freeze([
  'language/expressions/addition',
  'language/expressions/equals',
  'language/expressions/does-not-equals',
  'language/expressions/strict-equals',
  'language/expressions/strict-does-not-equals',
  'language/expressions/typeof',
  'language/expressions/logical-and',
  'language/expressions/logical-or',
  'language/expressions/conditional',
  'language/expressions/comma',
  'language/statements/let',
  'language/statements/const',
  'language/statements/for',
  'language/statements/for-of',
  'language/statements/labeled',
  'language/statements/switch',
  'language/statements/try',
  'built-ins/Number/prototype/toFixed',
  'built-ins/Number/prototype/toString',
  'built-ins/String/prototype/charAt',
  'built-ins/String/prototype/slice',
  'built-ins/String/prototype/indexOf',
  'built-ins/String/prototype/padStart',
  'built-ins/Math/abs',
  'built-ins/Math/floor',
  'built-ins/Math/max',
  'built-ins/Math/min',
  'built-ins/Array/prototype/push',
  'built-ins/Array/prototype/map',
  'built-ins/Array/prototype/filter',
  'built-ins/Array/prototype/indexOf',
  'built-ins/JSON/stringify',
  'built-ins/JSON/parse',
]);

// Default-deny feature list, as the profile is (compiler/profile/manifest.json, default "deny"). Each tag is the profile row
// that promises it: L01 let/const and loops, L02 arrow functions, L04 destructuring, computed keys and own enumeration
// order, L06 optional chaining and ??, L08 exponentiation, L10 arrays and for-of, L11 Map/Set, L12 Symbol and the iterator
// and toPrimitive symbols (stdlib entries P), L13 optional catch binding, L09 well-formed JSON.stringify, L18 standard library.
// Everything else is excluded, for example generators (L15 excluded), Proxy and Reflect (L20), BigInt (VN4005), classes
// (L05), Symbol.species (stdlib X), TypedArray (VN4014), async functions (asynchronous completion is not observed here),
// tail calls, explicit resource management and cross-realm tests.
export const SUPPORTED_FEATURES = Object.freeze(new Set([
  'let',
  'const',
  'arrow-function',
  'destructuring-binding',
  'destructuring-assignment',
  'object-rest',
  'computed-property-names',
  'for-of',
  'for-in-order',
  'optional-catch-binding',
  'optional-chaining',
  'coalesce-expression',
  'exponentiation',
  'Symbol',
  'Symbol.iterator',
  'Symbol.toPrimitive',
  'Map',
  'Set',
  'Array.prototype.includes',
  'well-formed-json-stringify',
]));

// Flags: "generated" is informational. "onlyStrict" runs the strict variant, which is the only variant this runner has
// (modules are strict). Sloppy-only (noStrict), module, async, raw and CanBlock flags are excluded.
export const SUPPORTED_FLAGS = Object.freeze(new Set(['generated', 'onlyStrict']));

// Step 2 of the selection: the size of the sample per listed directory (about 300 files in total).
export const MAX_PER_DIRECTORY = 10;

// Harness files a selected test may include. assert.js and sta.js are always prepended (test262 requires them).
// propertyHelper.js is not vendored: verifyProperty uses Object.defineProperty, which the profile rejects (VN4017).
export const SUPPORTED_INCLUDES = Object.freeze(new Set(['compareArray.js']));
export const ALWAYS_INCLUDED = Object.freeze(['assert.js', 'sta.js']);

// Negative tests: "parse" phase with SyntaxError (early errors), "runtime" phase with an error type a module can throw.
// Resolution-phase tests are module tests and are excluded with the module flag.
const NEGATIVE_RULES = Object.freeze({ parse: ['SyntaxError'], runtime: ['ReferenceError', 'TypeError', 'RangeError'] });

// The first rule a test breaks, as a short reason, or null when it is selected.
export function reasonToExclude(meta, { isFixture = false } = {}) {
  if (isFixture) return 'module fixture (not a test)';
  for (const flag of meta.flags) if (!SUPPORTED_FLAGS.has(flag)) return `flag ${flag}`;
  for (const feature of meta.features) if (!SUPPORTED_FEATURES.has(feature)) return `feature ${feature}`;
  for (const include of meta.includes) if (!SUPPORTED_INCLUDES.has(include)) return `include ${include}`;
  if (meta.negative !== null) {
    const types = NEGATIVE_RULES[meta.negative.phase];
    if (!types) return `negative phase ${meta.negative.phase}`;
    if (!types.includes(meta.negative.type)) return `negative ${meta.negative.phase}/${meta.negative.type}`;
  }
  return null;
}

// Walks the listed directories of a checkout's test/ (sorted, deterministic) and returns test paths relative to testRoot.
export function walkTests(testRoot, dirs = DIRECTORIES) {
  const out = [];
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.isFile() && entry.name.endsWith('.js')) out.push(path.relative(testRoot, full).split(path.sep).join('/'));
    }
  };
  for (const d of dirs) {
    const full = path.join(testRoot, d);
    if (!fs.existsSync(full)) throw new Error(`listed directory is missing from the checkout: test/${d}`);
    walk(full);
  }
  return out;
}

// Systematic sample of at most `max` entries of a sorted list: entry i of max is at floor(i * n / max).
export function sampleEvenly(sorted, max = MAX_PER_DIRECTORY) {
  const n = sorted.length;
  if (n <= max) return sorted.slice();
  return Array.from({ length: max }, (_, i) => sorted[Math.floor((i * n) / max)]);
}

// The selection over a checkout's test/: { selected, excluded, unreadable, eligibleByDirectory, sampledOut }.
// selected is sorted by path; excluded maps each first failing rule to its count; unreadable lists files with bad metadata.
export function selectFrom(testRoot, dirs = DIRECTORIES) {
  const excluded = new Map();
  const unreadable = [];
  const bump = (reason) => excluded.set(reason, (excluded.get(reason) ?? 0) + 1);
  const eligible = new Map(dirs.map((d) => [d, []]));
  for (const rel of walkTests(testRoot, dirs)) {
    const isFixture = rel.endsWith('_FIXTURE.js');
    let meta;
    try {
      meta = parseMeta(fs.readFileSync(path.join(testRoot, rel), 'utf8'));
    } catch (err) {
      if (!(err instanceof MetaError)) throw err;
      if (isFixture) bump('module fixture (not a test)');
      else {
        unreadable.push(rel);
        bump(`unreadable metadata: ${err.message}`);
      }
      continue;
    }
    const reason = reasonToExclude(meta, { isFixture });
    if (reason !== null) {
      bump(reason);
      continue;
    }
    const owner = dirs.find((d) => rel.startsWith(`${d}/`));
    eligible.get(owner).push(rel);
  }
  const selected = [];
  const eligibleByDirectory = new Map();
  for (const d of dirs) {
    const list = eligible.get(d).sort();
    eligibleByDirectory.set(d, list.length);
    selected.push(...sampleEvenly(list));
  }
  const sampledOut = [...eligibleByDirectory.values()].reduce((a, b) => a + b, 0) - selected.length;
  if (sampledOut > 0) excluded.set(`not sampled (more than ${MAX_PER_DIRECTORY} eligible in a directory)`, sampledOut);
  return { selected: selected.sort(), excluded, unreadable, eligibleByDirectory };
}

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

// The README that --write puts in the vendor directory.
export function readmeText({ commit, selectedCount, harnessFiles, throwsTests }) {
  return [
    '# Test262 subset (WP 3.1)',
    '',
    `Upstream: https://github.com/tc39/test262 at commit \`${commit}\` (a shallow checkout; the files are copied unchanged).`,
    'License: BSD-3-Clause, see LICENSE in this directory (copied from the upstream root).',
    '',
    `Vendored: ${selectedCount} test files under test/ and ${harnessFiles.length} harness files under harness/ (${harnessFiles.join(', ')}).`,
    'Only the files these rules select are vendored, plus the harness files they include.',
    "controls/ holds the runner's negative control (a copy of a passing test with a wrong expected value). It is not a selected test.",
    '',
    '## Selection rules',
    '',
    'Implemented in tooling/harness/select-test262.mjs and applied to the metadata of every file in these directories:',
    '',
    ...DIRECTORIES.map((d) => `- test/${d}`),
    '',
    '1. Fixture modules (`*_FIXTURE.js`) are not tests.',
    `2. Flags must be in: ${[...SUPPORTED_FLAGS].join(', ')}. Sloppy-only, module, async and raw tests are excluded.`,
    '   The runner compiles every test as a module, so only the strict variant runs. A test with no flag is run in strict',
    '   mode only; test262 also defines a sloppy variant for it, which is not run.',
    `3. Every feature must be in the supported list: ${[...SUPPORTED_FEATURES].join(', ')}.`,
    `4. Includes must be in: ${[...SUPPORTED_INCLUDES].join(', ')} (assert.js and sta.js are always prepended).`,
    `5. Negative tests: parse phase with ${NEGATIVE_RULES.parse.join(' or ')}; runtime phase with ${NEGATIVE_RULES.runtime.join(', ')}.`,
    '6. The metadata must be readable by tooling/harness/test262-meta.mjs.',
    '',
    '## Adaptation',
    '',
    'Every file here except controls/ is byte-identical to upstream. The runner (tooling/harness/run-test262.mjs, `adaptForProfile`) changes',
    'the module text it compiles, in these ways only: `new Test262Error(` becomes `Test262Error(` everywhere; in sta.js the',
    'Test262Error constructor becomes a function that returns `{ constructor: Test262Error, message }`, and its',
    'prototype.toString line is removed. Reason: `new` of a function the program defines is outside the language profile',
    '(L05, VN2007), and every assertion of the harness constructs its error that way; a function the program defines has no',
    `prototype object in the runtime. The \`constructor\` property keeps \`assert.throws(Test262Error, ...)\` (${throwsTests} selected test files)`,
    'meaning what it means upstream. Other constructs are not rewritten: a test that needs them is rejected by the profile.',
    '',
    'Run: `node tooling/harness/run-test262.mjs` (ledger: status.json). Check this tree against an upstream checkout:',
    '`node tooling/harness/select-test262.mjs --upstream <checkout> --check`.',
    '',
  ].join('\n');
}

function main(argv) {
  const at = argv.indexOf('--upstream');
  if (at < 0 || argv[at + 1] === undefined) throw new Error('usage: select-test262.mjs --upstream <checkout> [--write | --check]');
  const upstream = path.resolve(argv[at + 1]);
  const testRoot = path.join(upstream, 'test');
  const { selected, excluded, unreadable, eligibleByDirectory } = selectFrom(testRoot);
  // The harness files the selected tests include, in addition to the always-prepended ones.
  const included = new Set();
  for (const rel of selected) for (const h of parseMeta(fs.readFileSync(path.join(testRoot, rel), 'utf8')).includes) included.add(h);
  const harnessFiles = [...ALWAYS_INCLUDED, ...[...SUPPORTED_INCLUDES].filter((h) => included.has(h))];

  console.log(`selected ${selected.length} test files (${unreadable.length} unreadable metadata)`);
  for (const d of DIRECTORIES) {
    const n = selected.filter((rel) => rel.startsWith(`${d}/`)).length;
    console.log(`  ${String(n).padStart(4)} of ${String(eligibleByDirectory.get(d)).padStart(4)} eligible  test/${d}`);
  }
  console.log('excluded, by first failing rule:');
  for (const [reason, n] of [...excluded].sort((a, b) => b[1] - a[1])) console.log(`  ${String(n).padStart(5)}  ${reason}`);

  const files = new Map();
  for (const rel of selected) files.set(`test/${rel}`, fs.readFileSync(path.join(testRoot, rel)));
  for (const h of harnessFiles) files.set(`harness/${h}`, fs.readFileSync(path.join(upstream, 'harness', h)));
  files.set('LICENSE', fs.readFileSync(path.join(upstream, 'LICENSE')));
  const commit = execFileSync('git', ['-C', upstream, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
  const throwsTests = selected.filter((rel) => /assert\.throws\(\s*Test262Error/.test(fs.readFileSync(path.join(testRoot, rel), 'utf8'))).length;

  if (argv.includes('--write')) {
    // controls/ is not generated here, so it survives a rewrite: only the generated parts are replaced.
    for (const part of ['test', 'harness', 'LICENSE', 'README.md']) fs.rmSync(path.join(VENDOR_DIR, part), { recursive: true, force: true });
    for (const [rel, bytes] of files) {
      const dest = path.join(VENDOR_DIR, rel);
      fs.mkdirSync(path.dirname(dest), { recursive: true });
      fs.writeFileSync(dest, bytes);
    }
    fs.writeFileSync(path.join(VENDOR_DIR, 'README.md'), readmeText({ commit, selectedCount: selected.length, harnessFiles, throwsTests }));
    console.log(`vendored ${selected.length} tests and ${harnessFiles.length} harness files into ${path.relative(ROOT, VENDOR_DIR)} at ${commit}`);
  } else if (argv.includes('--check')) {
    const problems = [];
    const have = new Map();
    const walk = (dir, prefix) => {
      if (!fs.existsSync(dir)) return;
      for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, e.name);
        if (e.isDirectory()) walk(full, `${prefix}${e.name}/`);
        else if (e.isFile() && !(prefix === '' && e.name === 'README.md')) have.set(`${prefix}${e.name}`, fs.readFileSync(full));
      }
    };
    walk(VENDOR_DIR, '');
    // controls/ holds the runner's negative control (not a selected test); it is run on its own, never checked here.
    // status.json is the runner's ledger, written by run-test262.mjs, not part of the selection.
    for (const rel of [...have.keys()]) if (rel.startsWith('controls/') || rel === 'status.json') have.delete(rel);
    for (const [rel, bytes] of files) {
      if (!have.has(rel)) problems.push(`missing ${rel}`);
      else if (!have.get(rel).equals(bytes)) problems.push(`differs from upstream: ${rel}`);
    }
    for (const rel of have.keys()) if (!files.has(rel)) problems.push(`not selected by the rules: ${rel}`);
    const readmePath = path.join(VENDOR_DIR, 'README.md');
    if (!fs.existsSync(readmePath) || !fs.readFileSync(readmePath, 'utf8').includes(commit)) problems.push(`README does not name commit ${commit}`);
    for (const p of problems) console.log(`MISMATCH ${p}`);
    console.log(`check: ${files.size} expected files (tests, harness, LICENSE), ${have.size} vendored, ${problems.length} mismatch(es) at ${commit}`);
    console.log(`LICENSE sha256 ${sha256(files.get('LICENSE'))}`);
    process.exitCode = problems.length === 0 ? 0 : 1;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main(process.argv.slice(2));
