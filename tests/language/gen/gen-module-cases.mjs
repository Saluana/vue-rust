// Generates tests/language/corpora/modules.json: the behaviour of ES modules under Node's own loader (Node
// >= 22, no dependencies). Each case writes its .mjs files into a scratch directory, loads files in the listed
// steps, and records per step: the outcome ("ok" or the thrown error's name), the evaluation log written by
// that step, and whether a thrown value is the same object as one thrown earlier in the case.
// Deterministic: fixed case list, no timestamps, no absolute paths.
// Usage: node tests/language/gen/gen-module-cases.mjs [--check]
//   --check  regenerates in memory and exits 1 when the committed corpus is different.

import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const OUT = fileURLToPath(new URL('../corpora/modules.json', import.meta.url));
const PRELUDE = "const L = (s) => globalThis.__vnLog.push(s);\n";

// Each case: files (name -> body; the prelude is prepended) and steps (files to import, in order).
const CASES = [
  { name: 'single', files: { 'main.mjs': "L('main');" }, steps: ['main.mjs'] },
  {
    name: 'chain',
    files: {
      'main.mjs': "import './b.mjs';\nL('main');",
      'b.mjs': "import './c.mjs';\nL('b');",
      'c.mjs': "L('c');",
    },
    steps: ['main.mjs'],
  },
  {
    name: 'diamond',
    files: {
      'main.mjs': "import './a.mjs';\nimport './b.mjs';\nL('main');",
      'a.mjs': "import './d.mjs';\nL('a');",
      'b.mjs': "import './d.mjs';\nL('b');",
      'd.mjs': "L('d');",
    },
    steps: ['main.mjs'],
  },
  {
    name: 'cycle-function',
    files: {
      'main.mjs': "import { g } from './b.mjs';\nexport function f() { return 'f'; }\nL('main:' + g());",
      'b.mjs': "import { f } from './main.mjs';\nexport function g() { return 'g' + f(); }\nL('b:' + f());",
    },
    steps: ['main.mjs'],
  },
  {
    name: 'cycle-let-uncaught',
    files: {
      'main.mjs': "import './b.mjs';\nexport let v = 1;\nL('main');",
      'b.mjs': "import { v } from './main.mjs';\nL('b:' + v);",
    },
    steps: ['main.mjs', 'main.mjs'],
  },
  {
    name: 'cycle-let-caught',
    files: {
      'main.mjs': "import './b.mjs';\nexport let v = 1;\nL('main:' + v);",
      'b.mjs': "import { v } from './main.mjs';\ntry { L('b:' + v); } catch (e) { L('b:' + e.name); }",
    },
    steps: ['main.mjs'],
  },
  {
    name: 'reexport-rename',
    files: {
      'main.mjs': "import { y } from './a.mjs';\nL('main:' + y);",
      'a.mjs': "export { x as y } from './b.mjs';",
      'b.mjs': "export const x = 42;\nL('b');",
    },
    steps: ['main.mjs'],
  },
  {
    name: 'reexport-chain',
    files: {
      'main.mjs': "import { z } from './a.mjs';\nL('main:' + z);",
      'a.mjs': "export { y as z } from './b.mjs';",
      'b.mjs': "export { x as y } from './c.mjs';",
      'c.mjs': "export const x = 'cx';\nL('c');",
    },
    steps: ['main.mjs'],
  },
  {
    name: 'star-same-binding',
    files: {
      'main.mjs': "import { x } from './a.mjs';\nL('main:' + x);",
      'a.mjs': "export * from './b.mjs';\nexport * from './c.mjs';",
      'b.mjs': "export * from './d.mjs';",
      'c.mjs': "export * from './d.mjs';",
      'd.mjs': "export const x = 'dx';\nL('d');",
    },
    steps: ['main.mjs'],
  },
  {
    name: 'star-ambiguous-namespace',
    files: {
      'main.mjs': "import * as ns from './a.mjs';\nL('keys:' + Object.keys(ns).join(','));\nL('x:' + ('x' in ns));",
      'a.mjs': "export * from './b.mjs';\nexport * from './c.mjs';",
      'b.mjs': "export const x = 1, y = 2;\nL('b');",
      'c.mjs': "export const x = 3, z = 4;\nL('c');",
    },
    steps: ['main.mjs'],
  },
  {
    name: 'star-ambiguous-import',
    files: {
      'main.mjs': "import { x } from './a.mjs';\nL('main');",
      'a.mjs': "export * from './b.mjs';\nexport * from './c.mjs';",
      'b.mjs': "export const x = 1, y = 2;\nL('b');",
      'c.mjs': "export const x = 3, z = 4;\nL('c');",
    },
    steps: ['main.mjs', 'b.mjs'],
  },
  {
    name: 'star-default-excluded',
    files: {
      'main.mjs': "import * as ns from './a.mjs';\nL('keys:' + Object.keys(ns).join(','));\nL('default:' + ('default' in ns));",
      'a.mjs': "export * from './b.mjs';",
      'b.mjs': "export default 'bd';\nexport const q = 2;",
    },
    steps: ['main.mjs'],
  },
  {
    name: 'star-cycle',
    files: {
      'main.mjs': "import * as ns from './a.mjs';\nL('keys:' + Object.keys(ns).join(','));",
      'a.mjs': "export * from './b.mjs';\nexport const a1 = 1;",
      'b.mjs': "export * from './a.mjs';\nexport const b1 = 2;",
    },
    steps: ['main.mjs'],
  },
  {
    name: 'missing-module',
    files: { 'main.mjs': "import './nope.mjs';\nL('main');" },
    steps: ['main.mjs'],
  },
  {
    name: 'missing-export',
    files: {
      'main.mjs': "import { nope } from './lib.mjs';\nL('main');",
      'lib.mjs': "export const yes = 1;\nL('lib');",
    },
    steps: ['main.mjs', 'lib.mjs'],
  },
  {
    name: 'error-cache',
    files: {
      'main.mjs': "import './bad.mjs';\nL('main');",
      'bad.mjs': "L('bad');\nthrow new RangeError('boom');",
    },
    steps: ['main.mjs', 'main.mjs'],
  },
  {
    name: 'error-sibling',
    files: {
      'main.mjs': "import './bad.mjs';\nimport './other.mjs';\nL('main');",
      'bad.mjs': "L('bad');\nthrow new RangeError('boom');",
      'other.mjs': "L('other');",
    },
    steps: ['main.mjs', 'other.mjs', 'main.mjs'],
  },
  {
    name: 'error-chain-cache',
    files: {
      'main.mjs': "import './mid.mjs';\nL('main');",
      'mid.mjs': "import './bad.mjs';\nL('mid');",
      'bad.mjs': "L('bad');\nthrow new RangeError('boom');",
    },
    steps: ['main.mjs', 'mid.mjs'],
  },
  {
    name: 'native-vue',
    files: {
      'main.mjs': "import { version } from 'vue';\nL('main:' + version);",
      'node_modules/vue/package.json': '{"name":"vue","version":"3.5.43","type":"module","exports":"./index.mjs"}',
      'node_modules/vue/index.mjs': "export const version = '3.5.43';",
    },
    steps: ['main.mjs'],
  },
  {
    name: 'namespace-contents',
    files: {
      'lib.mjs': [
        'export const a = 1;',
        'export let b = 2;',
        'export let counter = 0;',
        'export function bump() { counter++; }',
        "export function f() { return 'f'; }",
        'const hidden = 3;',
        'export { hidden as zz };',
        'export { a as "10", b as "2" };',
        "export default 'dflt';",
      ].join('\n'),
      'main.mjs': [
        "import * as ns from './lib.mjs';",
        "import * as ns2 from './lib.mjs';",
        "import { bump } from './lib.mjs';",
        "L('keys:' + Object.keys(ns).join(','));",
        "L('own:' + Reflect.ownKeys(ns).map((k) => typeof k === 'symbol' ? 'sym:' + k.description : k).join(','));",
        "L('proto:' + (Object.getPrototypeOf(ns) === null));",
        "L('ext:' + Object.isExtensible(ns));",
        "L('tag:' + String(ns[Symbol.toStringTag]));",
        "L('vals:' + [ns['10'], ns['2'], ns.a, ns.b, ns.zz, ns.default, ns.f(), ns.counter, ns.missing].map(String).join(','));",
        "const d = Object.getOwnPropertyDescriptor(ns, 'a');",
        "L('desc:' + [d.value, d.writable, d.enumerable, d.configurable].join(','));",
        "const t = Object.getOwnPropertyDescriptor(ns, Symbol.toStringTag);",
        "L('desctag:' + [t.value, t.writable, t.enumerable, t.configurable].join(','));",
        "L('set:' + Reflect.set(ns, 'a', 5));",
        "L('define:' + Reflect.defineProperty(ns, 'a', { value: 9 }));",
        "L('delete:' + Reflect.deleteProperty(ns, 'a'));",
        "L('delete2:' + Reflect.deleteProperty(ns, 'nope'));",
        "L('has:' + ['a' in ns, 'nope' in ns, 'zz' in ns].join(','));",
        "L('same:' + (ns === ns2));",
        "try { ns.a = 2; L('assign:none'); } catch (e) { L('assign:' + e.name); }",
        "L('seal:' + (Object.seal(ns) === ns));",
        "try { Object.freeze(ns); L('freeze:none'); } catch (e) { L('freeze:' + e.name); }",
        'bump();',
        "L('live:' + ns.counter);",
      ].join('\n'),
    },
    steps: ['main.mjs'],
  },
  {
    name: 'namespace-tdz',
    files: {
      'lib.mjs': "import './probe.mjs';\nexport let v = 1;\nL('lib');",
      'probe.mjs': [
        "import * as ns from './lib.mjs';",
        "L('in:' + ('v' in ns));",
        "try { Object.keys(ns); L('keys:ok'); } catch (e) { L('keys:' + e.name); }",
        "try { L('read:' + ns.v); } catch (e) { L('tdz:' + e.name); }",
      ].join('\n'),
      'main.mjs': "import './lib.mjs';\nL('main');",
    },
    steps: ['main.mjs'],
  },
  {
    name: 'live-binding',
    files: {
      'lib.mjs': 'export let count = 0;\nexport function bump() { count++; }',
      'main.mjs': "import { count, bump } from './lib.mjs';\nL('before:' + count);\nbump();\nL('after:' + count);",
    },
    steps: ['main.mjs'],
  },
];

async function runCase(root, testCase) {
  const dir = join(root, testCase.name);
  for (const [file, body] of Object.entries(testCase.files)) {
    const path = join(dir, file);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, file.endsWith('.json') ? body : PRELUDE + body + '\n');
  }
  const thrown = [];
  const steps = [];
  for (const file of testCase.steps) {
    globalThis.__vnLog = [];
    let outcome = 'ok';
    let error;
    let failed = false;
    try {
      await import(pathToFileURL(join(dir, file)).href);
    } catch (e) {
      failed = true;
      error = e;
      outcome = e !== null && typeof e === 'object' && typeof e.name === 'string' ? e.name : 'non-error';
    }
    const sameThrow = failed ? thrown.some((x) => x === error) : null;
    if (failed) thrown.push(error);
    steps.push({ load: file, outcome, sameThrow, log: globalThis.__vnLog.slice() });
  }
  return { name: testCase.name, steps };
}

async function generate() {
  const root = mkdtempSync(join(tmpdir(), 'vn-modules-'));
  try {
    const cases = [];
    for (const testCase of CASES) cases.push(await runCase(root, testCase));
    return JSON.stringify({ cases }, null, 2) + '\n';
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

const text = await generate();
if (process.argv.includes('--check')) {
  const committed = readFileSync(OUT, 'utf8');
  if (committed !== text) {
    console.error(`modules corpus is stale: regenerate with node tests/language/gen/gen-module-cases.mjs`);
    process.exit(1);
  }
  console.log(`modules corpus up to date (${CASES.length} cases)`);
} else {
  writeFileSync(OUT, text);
  console.log(`wrote ${OUT} (${CASES.length} cases)`);
}
