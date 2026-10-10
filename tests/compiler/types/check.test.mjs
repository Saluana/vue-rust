// The type checker (task 5.7) on its own, compiler/types/check.mjs: the pinned pair checks a project of .vue and .ts
// modules, each error is a VN7001 at the .vue or .ts span it came from, JavaScript is not checked, vue comes from the
// framework, DOM is in the web library only, and a checkout with another tooling pair is refused. Every expected line and
// column is counted by hand from the source text that the test writes or reads.
import { test, describe } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { TypeCheckError, typeCheckProject } from '../../../compiler/types/check.mjs';
import { fakeCheckout, writeProject } from '../../scratch.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
const PAIR = path.join(ROOT, 'tests', 'compiler', 'types', 'fixtures', 'pair');
const PINNED_VERSIONS = {
  typescript: '6.0.3', 'vue-tsc': '3.3.12', '@vue/language-core': '3.3.12', '@volar/typescript': '2.4.28', vue: '3.5.43',
};
const QUICK_QUOTE = ['/src/App.vue', '/src/FieldInput.vue', '/src/LineItem.vue', '/src/money.ts'];

function check(root, modules, target = 'web') {
  return typeCheckProject({ root, modules, entry: modules[0], target, frameworkRoot: ROOT });
}

// file:line:column:severity:code for each diagnostic, the form the expectations below are written in.
const where = (result) => result.diagnostics.map((d) => `${d.file}:${d.start.line}:${d.start.column}:${d.severity}:${d.code}`);

// The same component in JavaScript and in TypeScript: `count.value = 'three'` is a type error only in TypeScript.
const componentSource = (lang) => `<script setup${lang ? ` lang="${lang}"` : ''}>
import { ref } from 'vue'
const count = ref(0)
count.value = 'three'
</script>
<template><p>{{ count }}</p></template>
`;

describe('the pinned pair on the probe fixture', () => {
  test('the two known mistakes are reported at their .vue spans; the child component is clean', () => {
    const r = check(PAIR, ['/src/App.vue', '/src/Child.vue']);
    assert.deepEqual(r.versions, PINNED_VERSIONS);
    // App.vue line 6 is `count.value = 'three'`, so the error starts at column 0. Line 11 is `  <Child :label="3" />`,
    // where the attribute `:label` starts at column 9 and `label` at column 10.
    assert.deepEqual(where(r), ['src/App.vue:6:0:error:VN7001', 'src/App.vue:11:10:error:VN7001']);
    assert.equal(r.diagnostics[0].message, "TS2322: Type 'string' is not assignable to type 'number'.");
    assert.equal(r.diagnostics[1].message, "TS2322: Type 'number' is not assignable to type 'string'.");
  });

  test('the examples are clean: Counter, and all four modules of Quick Quote', () => {
    assert.deepEqual(where(check(path.join(ROOT, 'examples', 'counter'), ['/src/App.vue'])), []);
    assert.deepEqual(where(check(path.join(ROOT, 'examples', 'quick-quote'), QUICK_QUOTE)), []);
  });

  test('no modules: nothing to report, and the pinned versions are still returned', () => {
    assert.deepEqual(typeCheckProject({ root: PAIR, modules: [], frameworkRoot: ROOT }), {
      versions: PINNED_VERSIONS, diagnostics: [],
    });
  });
});

describe('JavaScript is not type checked, TypeScript is', () => {
  test('a JavaScript component and a JavaScript module with type-incorrect code report nothing (checkJs is off)', () => {
    const root = writeProject('check-js', {
      'src/App.vue': componentSource(''),
      'src/util.js': "export const total = 'a'\nexport const n = total.toFixed(2)\n",
    });
    assert.deepEqual(where(check(root, ['/src/App.vue', '/src/util.js'])), []);
  });

  test('negative control: the same code written in TypeScript is reported', () => {
    const root = writeProject('check-ts', {
      'src/App.vue': componentSource('ts'),
      'src/util.ts': "export const total: string = 'a'\nexport const n = total.toFixed(2)\n",
    });
    // App.vue line 4 starts at column 0 with `count.value`. util.ts line 2 has `toFixed` at column 23.
    assert.deepEqual(where(check(root, ['/src/App.vue', '/src/util.ts'])), [
      'src/App.vue:4:0:error:VN7001',
      'src/util.ts:2:23:error:VN7001',
    ]);
  });
});

describe('modules resolve as they would at run time', () => {
  test('vue comes from the framework for a project outside it; an unknown package is TS2307 at its specifier', () => {
    const root = writeProject('check-resolve', {
      'src/App.vue': "<script setup lang=\"ts\">\nimport { ref } from 'vue'\nimport { x } from 'no-such-pkg'\nconst n = ref(1)\n</script>\n<template><p>{{ n }}</p></template>\n",
    });
    // Line 3 is `import { x } from 'no-such-pkg'`. The module name starts at column 18, quotes included.
    const r = check(root, ['/src/App.vue']);
    assert.deepEqual(where(r), ['src/App.vue:3:18:error:VN7001']);
    assert.match(r.diagnostics[0].message, /^TS2307: Cannot find module 'no-such-pkg'/);
  });

  test('an error in a .ts module imported by a component is reported in that module', () => {
    const root = writeProject('check-cross-file', {
      'src/App.vue': "<script setup lang=\"ts\">\nimport { money } from './money.ts'\nconst s: number = money(2)\n</script>\n<template><p>{{ s }}</p></template>\n",
      'src/money.ts': 'export function money(n: number): string {\n  return n * 2\n}\n',
    });
    // App.vue line 3: `s` at column 6 is declared number, but money returns string. money.ts line 2: `return` at column 2.
    assert.deepEqual(where(check(root, ['/src/App.vue', '/src/money.ts'])), [
      'src/App.vue:3:6:error:VN7001',
      'src/money.ts:2:2:error:VN7001',
    ]);
  });

  test('DOM is in the web library only: document is TS2584 on the native target', () => {
    const root = writeProject('check-dom', {
      'src/App.vue': "<script setup lang=\"ts\">\nconst title: string = document.title\n</script>\n<template><p>{{ title }}</p></template>\n",
    });
    assert.deepEqual(where(check(root, ['/src/App.vue'], 'web')), []);
    // Line 2 is `const title: string = document.title`. `document` starts at column 22.
    const native = check(root, ['/src/App.vue'], 'native');
    assert.deepEqual(where(native), ['src/App.vue:2:22:error:VN7001']);
    assert.match(native.diagnostics[0].message, /^TS2584: Cannot find name 'document'\./);
  });
});

describe('negative control: a mutated example fails at the mutated span', () => {
  test('Counter with `count.value = 0` changed to a string is reported at that line and column', () => {
    const original = fs.readFileSync(path.join(ROOT, 'examples', 'counter', 'src', 'App.vue'), 'utf8');
    const line = '  count.value = 0\n';
    assert.equal(original.split(line).length, 2, 'the line occurs once in the example');
    const root = writeProject('check-mutated-counter', { 'src/App.vue': original.replace(line, "  count.value = 'zero'\n") });
    // Line 12 of the Counter's App.vue becomes `  count.value = 'zero'`. `count.value` starts at column 2.
    assert.deepEqual(where(check(root, ['/src/App.vue'])), ['src/App.vue:12:2:error:VN7001']);
  });
});

describe('the tooling pair is required', () => {
  test('a checkout with TypeScript 7 is refused: TypeCheckError, and nothing is checked', () => {
    const checkout = fakeCheckout('ts7', {
      devDependencies: { typescript: '6.0.3', vue: '3.5.43', 'vue-tsc': '3.3.12' },
      installed: { typescript: '7.0.2' },
    });
    assert.throws(
      () => typeCheckProject({ root: PAIR, modules: ['/src/App.vue'], entry: '/src/App.vue', target: 'web', frameworkRoot: checkout }),
      (err) => err instanceof TypeCheckError && /TypeScript 7\.0\.2 is installed/.test(err.message),
    );
  });

  test('bad arguments are TypeErrors: a relative root and an unknown target', () => {
    assert.throws(() => typeCheckProject({ root: 'src', modules: ['/src/App.vue'], frameworkRoot: ROOT }), TypeError);
    assert.throws(() => typeCheckProject({ root: PAIR, modules: ['/src/App.vue'], target: 'bogus', frameworkRoot: ROOT }), TypeError);
  });
});
