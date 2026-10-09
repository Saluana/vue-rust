// WP-V0 tests: the Vue scenario corpus (tests/vue/cases) against its goldens. The goldens are the Node oracle's output
// for the production build of vue@3.5.43. Expected values below are hand-written, or come from the oracle itself, and
// each check has a negative control showing that it can fail.
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {cp, mkdtemp, readFile, rm, writeFile} from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {parse} from '@babel/parser';
import {isStdlibSupported, listSupported} from '../../compiler/profile/stdlib.mjs';
import {compareWithGolden, listCases} from '../../tooling/oracle/check-goldens.mjs';
import {
  VUE_MEMBER_CALLS,
  VUE_NODE_ENV,
  checkVueGoldens,
  checkVueHeader,
  isVueMemberCall,
  lintVueFile,
  lintVueSource,
  runVueScenarioLine,
  vueCasesRoot,
  vueGoldenRoot,
  vueImportAllowlist,
  vueScenarioName,
} from '../../tooling/oracle/vue-env.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '../..');
const relOf = (file) => path.relative(vueCasesRoot, file).split(path.sep).join('/');
const caseFile = (rel) => path.join(vueCasesRoot, `${rel}.js`);
const V_IDS = Array.from({length: 17}, (_, i) => `V${String(i + 1).padStart(2, '0')}`);

// Number of scenarios that list each requirement ID on their "// @covers" line.
function coverageOf(headerLines) {
  const count = new Map();
  for (const line of headerLines) {
    for (const id of line.replace('// @covers ', '').split(',')) count.set(id, (count.get(id) ?? 0) + 1);
  }
  return count;
}

// Audit: every ID needs at least `min` covering scenarios. Returns the failing IDs (empty when the audit passes).
function coverageProblems(ids, headerLines, min) {
  const count = coverageOf(headerLines);
  return ids
    .filter((id) => (count.get(id) ?? 0) < min)
    .map((id) => `${id} covered by ${count.get(id) ?? 0} scenario(s)`);
}

async function headerLinesOf(files) {
  return Promise.all(files.map(async (f) => (await readFile(f, 'utf8')).split('\n')[1]));
}

// ---- production build and child environment ----

// Every name a scenario imports from 'vue', read from the cases themselves.
async function importedVueNames() {
  const names = new Set();
  for (const file of await listCases(vueCasesRoot)) {
    const ast = parse(await readFile(file, 'utf8'), {sourceType: 'module'});
    for (const node of ast.program.body) {
      if (node.type === 'ImportDeclaration' && node.source.value === 'vue') {
        for (const s of node.specifiers) names.add(s.imported.name);
      }
    }
  }
  return [...names].sort();
}

test('scenarios run the production build: the vue bindings a scenario imports are the production runtime (NODE_ENV=production)', async () => {
  const names = await importedVueNames();
  assert.ok(names.length >= 25, `${names.length} imported names`);
  // Compares each binding with the production runtime-core module. Node's ESM interop also evaluates the reexported
  // development modules while it reads vue's export names, but those are not the bindings a scenario receives.
  const mismatches = (env) => {
    const code = [
      "import * as vue from 'vue'",
      "import {createRequire} from 'node:module'",
      `const names = ${JSON.stringify(names)}`,
      "const core = createRequire(process.cwd() + '/')('@vue/runtime-core/dist/runtime-core.cjs.prod.js')",
      'console.log(JSON.stringify(names.filter((n) => vue[n] !== core[n])))',
    ].join('\n');
    const r = spawnSync(process.execPath, ['--input-type=module', '-e', code], {cwd: repo, env, encoding: 'utf8'});
    assert.equal(r.status, 0, r.stderr);
    return JSON.parse(r.stdout);
  };
  assert.deepEqual(mismatches({...process.env, NODE_ENV: VUE_NODE_ENV}), []);
  // negative control: without NODE_ENV=production the bindings come from the development runtime and are reported
  const devEnv = {...process.env};
  delete devEnv.NODE_ENV;
  assert.ok(mismatches(devEnv).includes('watch'));
});

test('runVueScenarioLine sets NODE_ENV for its child only: the parent environment is restored', async () => {
  const hadBefore = Object.prototype.hasOwnProperty.call(process.env, 'NODE_ENV');
  const valueBefore = process.env.NODE_ENV;
  delete process.env.NODE_ENV;
  try {
    const doc = JSON.parse(await runVueScenarioLine(caseFile('ref/get-set-primitive')));
    assert.equal(doc.scenario, 'ref/get-set-primitive');
    assert.equal(Object.prototype.hasOwnProperty.call(process.env, 'NODE_ENV'), false);
  } finally {
    if (hadBefore) process.env.NODE_ENV = valueBefore;
    else delete process.env.NODE_ENV;
  }
});

test('scenario names are the path under tests/vue/cases without .js; files outside that root are refused', () => {
  assert.equal(vueScenarioName(caseFile('watch/flush-order-pre-post-sync')), 'watch/flush-order-pre-post-sync');
  assert.throws(() => vueScenarioName('/elsewhere/probe.js'), /is not under/);
});

// ---- lint: header tags, imports, no Vue internals, and the language lint ----

test('lint: every case is clean (Vue header, imports from the allowlist, no Vue internals, language rules)', async () => {
  for (const file of await listCases(vueCasesRoot)) {
    assert.deepEqual(await lintVueFile(file), [], relOf(file));
  }
});

test('lint negative controls: each forbidden construct is reported with its code', () => {
  const head = '// @tier 1\n// @covers V01\n';
  const table = [
    ['effect is X in the export map', "import {effect} from 'vue'\n", 'VUE_API'],
    ['proxyRefs is X in the export map', "import {proxyRefs} from 'vue'\n", 'VUE_API'],
    ['component API', "import {h} from 'vue'\n", 'VUE_API'],
    ['namespace import of vue', "import * as vue from 'vue'\n", 'VUE_API'],
    ['foreign module', "import {ref} from 'react'\n", 'IMPORT'],
    ['reactive flag read', "import {ref} from 'vue'\ntrace('f', ref(1).__v_isRef)\n", 'VUE_INTERNAL'],
    ['ref internal dep read', "import {ref} from 'vue'\ntrace('d', ref(1).dep)\n", 'VUE_INTERNAL'],
    ['class syntax', 'class A {}\n', 'CLASS'],
    ['console', 'console.log(1)\n', 'FORBIDDEN_NAME'],
    ['bare setTimeout', 'setTimeout(() => {}, 1)\n', 'FORBIDDEN_NAME'],
    ['Object.defineProperty is not in the standard-library manifest', 'trace("x", Object.defineProperty({}, "a", {}))\n', 'UNLISTED_API'],
  ];
  const allowed = vueImportAllowlist();
  for (const [name, body, code] of table) {
    const codes = lintVueSource(head + body, allowed).map((p) => p.code);
    assert.ok(codes.includes(code), `${name}: expected ${code}, got ${codes.join(',')}`);
  }
});

// Method calls on Vue objects: the language lint matches method names only, so the wrapper exempts the Vue handle
// methods (VUE_MEMBER_CALLS) that the stdlib manifest does not govern. Everything else is still reported.
test('lint: calls to the Vue instance and handle methods pass by name (run, stop, pause, resume)', () => {
  const head = '// @tier 2\n// @covers V12,V15\n';
  const body = [
    "import {effectScope, ref, watch} from 'vue'",
    'const scope = effectScope()',
    'scope.run(() => 1)',
    'scope.stop()',
    'const handle = watch(ref(1), () => {})',
    'handle.pause()',
    'handle.resume()',
    'handle.stop()',
  ].join('\n');
  assert.deepEqual(lintVueSource(head + body + '\n').filter((p) => p.code === 'UNLISTED_API'), []);
  assert.deepEqual([...VUE_MEMBER_CALLS].sort(), ['pause', 'resume', 'run', 'stop']);
});

test('negative control: a method name that neither a builtin nor a Vue handle declares is still flagged', () => {
  const head = '// @tier 1\n// @covers V01,V12\n';
  const body = [
    "import {effectScope, ref, watch} from 'vue'", // line 3
    'const r = ref(1)', // 4
    'r.frobnicate()', // 5: unknown method, flagged
    'const scope = effectScope()', // 6
    'scope.on()', // 7: runtime-only on EffectScope, not in the public typings, flagged
    'const handle = watch(r, () => {})', // 8
    'handle.dispose()', // 9: unknown method, flagged
    'handle.stop()', // 10: allowed
    'scope.run(() => 1)', // 11: allowed
  ].join('\n');
  const flagged = lintVueSource(head + body + '\n').filter((p) => p.code === 'UNLISTED_API');
  assert.deepEqual(flagged.map((p) => p.line), [5, 7, 9]);
  assert.match(flagged[0].message, /\.frobnicate\(\)/);
});

test("the Vue member-call names are declared by Vue's public typings and are not stdlib method names", async () => {
  const dts = await readFile(path.join(repo, 'node_modules/@vue/reactivity/dist/reactivity.d.ts'), 'utf8');
  for (const decl of [
    /^\s+run<T>\(fn: \(\) => T\): T \| undefined;/m, // EffectScope
    /^\s+stop\(fromParent\?: boolean\): void;/m,
    /^\s+pause\(\): void;/m,
    /^\s+resume\(\): void;/m,
    /^\s+pause: \(\) => void;/m, // WatchHandle
    /^\s+resume: \(\) => void;/m,
    /^\s+stop: \(\) => void;/m,
  ]) {
    assert.match(dts, decl);
  }
  assert.doesNotMatch(dts, /^\s+(on|off)\(\)/m); // runtime-only, not public API
  const stdlib = new Set(
    listSupported('proto')
      .filter((e) => isStdlibSupported(e.path, 'test'))
      .map((e) => e.path.match(/\.([A-Za-z_$][\w$]*)$/)?.[1])
      .filter(Boolean),
  );
  assert.ok(stdlib.has('push') && stdlib.has('splice'), 'the derived stdlib method set is populated');
  for (const name of VUE_MEMBER_CALLS) assert.equal(stdlib.has(name), false, `${name} is a stdlib method name`);
  assert.equal(isVueMemberCall('stop'), true);
  assert.equal(isVueMemberCall('push'), false);
  assert.equal(isVueMemberCall('frobnicate'), false);
});

test('lint: the header rules accept Vue tags and reject language IDs, unknown tiers and missing tags', () => {
  const codes = (src) => checkVueHeader(src).map((p) => p.code);
  assert.deepEqual(codes('// @tier 1\n// @covers V01,V06\n'), []);
  assert.deepEqual(codes('// @tier 3\n// @covers V17\n'), []);
  assert.deepEqual(codes('// @tier 1\n// @covers L01\n'), ['HEADER']);
  assert.deepEqual(codes('// @tier 4\n// @covers V01\n'), ['HEADER']);
  assert.deepEqual(codes('// @tier 1\n// @covers V18\n'), ['HEADER']);
  assert.deepEqual(codes('trace(1)\n'), ['HEADER', 'HEADER']);
});

test('the import allowlist holds P-status V01..V17 exports only (no X export, no component or DOM API)', () => {
  const allowed = vueImportAllowlist();
  for (const name of ['ref', 'reactive', 'computed', 'watch', 'watchEffect', 'watchSyncEffect', 'effectScope', 'onWatcherCleanup', 'nextTick', 'toRaw']) {
    assert.ok(allowed.has(name), name);
  }
  for (const name of ['effect', 'stop', 'ReactiveEffect', 'proxyRefs', 'getCurrentWatcher', 'queuePostFlushCb', 'TrackOpTypes', 'h', 'createApp', 'onMounted', 'inject']) {
    assert.equal(allowed.has(name), false, name);
  }
});

// ---- goldens, determinism and negative controls ----

test('goldens match a fresh oracle run for every scenario (structurally and byte for byte), with no orphans', async () => {
  const {results, orphans} = await checkVueGoldens();
  assert.ok(results.length >= 90 && results.length <= 120, `case count ${results.length}`);
  assert.deepEqual(results.filter((r) => r.status !== 'ok').map((r) => [relOf(r.file), r.reason]), []);
  assert.deepEqual(orphans, []);
});

test('every case is deterministic: two child runs produce byte-identical output', async () => {
  for (const file of await listCases(vueCasesRoot)) {
    const a = await runVueScenarioLine(file);
    const b = await runVueScenarioLine(file);
    assert.equal(a, b, relOf(file));
  }
});

test('negative control: a doctored golden is reported at the first differing path; byte and missing-golden cases too', async () => {
  const actual = await runVueScenarioLine(caseFile('ref/get-set-primitive'));
  const doctored = actual.replace('"elems":[1,2,3]', '"elems":[1,2,4]');
  assert.notEqual(doctored, actual, 'the doctoring must change the golden text');
  assert.match(compareWithGolden(actual, doctored), /first difference at \$\.entries\[0\]\.values\[0\]\.elems\[2\]: 3 vs 4/);
  assert.equal(compareWithGolden(actual, actual), null);
  assert.match(compareWithGolden(actual, actual.replace('\n', ' \n')), /bytes differ/);
  assert.match(compareWithGolden(actual, null), /missing golden/);
});

test('negative control: checkVueGoldens reports a doctored golden file and an orphan golden file', async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), 'vn-vue-golden-'));
  try {
    await cp(vueGoldenRoot, dir, {recursive: true});
    const target = path.join(dir, 'ref', 'get-set-primitive.js.json');
    await writeFile(target, (await readFile(target, 'utf8')).replace('"elems":[1,2,3]', '"elems":[1,2,4]'));
    await writeFile(path.join(dir, 'ref', 'orphan.js.json'), '{}\n');
    const {results, orphans} = await checkVueGoldens({goldenRoot: dir});
    const failed = results.filter((r) => r.status !== 'ok');
    assert.deepEqual(failed.map((r) => relOf(r.file)), ['ref/get-set-primitive.js']);
    assert.match(failed[0].reason, /first difference at \$\.entries\[0\]\.values\[0\]\.elems\[2\]/);
    assert.deepEqual(orphans.map((g) => path.relative(dir, g)), [path.join('ref', 'orphan.js.json')]);
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});

// ---- scenario list audit ----

test('scenario list audit: every V01..V17 row of the inventory is covered by at least two scenarios', async () => {
  const inventory = JSON.parse(await readFile(path.join(repo, 'compiler/profile/inventory.generated.json'), 'utf8'));
  const ids = inventory.rows.map((r) => r.id).filter((id) => /^V(0[1-9]|1[0-7])$/.test(id));
  assert.deepEqual(ids, V_IDS);
  assert.deepEqual(coverageProblems(ids, await headerLinesOf(await listCases(vueCasesRoot)), 2), []);
});

test('negative control: the coverage audit reports each row with fewer than two covering scenarios', () => {
  const headers = ['// @covers V01,V02', '// @covers V01'];
  assert.deepEqual(coverageProblems(['V01', 'V02', 'V03'], headers, 2), ['V02 covered by 1 scenario(s)', 'V03 covered by 0 scenario(s)']);
});
