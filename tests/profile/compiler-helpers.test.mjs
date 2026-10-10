import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { isSupported } from '../../compiler/profile/index.mjs';
import { auditVueExports, parseExportMap } from '../../tooling/manifest/audit-vue-exports.mjs';
import { extractInventory } from '../../tooling/manifest/extract-inventory.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const requireFromRoot = createRequire(path.join(ROOT, 'package.json'));
const read = (rel) => readFileSync(path.join(ROOT, rel), 'utf8');
const designText = read('planning/vue-native-framework/design.md');
const exportMapText = read('compiler/profile/export-map.json');
const vueExports = JSON.parse(read('compiler/profile/vue-exports.json'));
const byName = new Map(vueExports.exports.map((entry) => [entry.name, entry]));

// Independent parse of the official compiler helper tables (this file does not use the audit module).
function helperTableNames() {
  const core = read('node_modules/@vue/compiler-core/dist/compiler-core.cjs.js');
  const dom = read('node_modules/@vue/compiler-dom/dist/compiler-dom.cjs.js');
  const sfc = read('node_modules/@vue/compiler-sfc/dist/compiler-sfc.cjs.js');
  const block = (source, start, end) => {
    const from = source.indexOf(start);
    assert.notEqual(from, -1, `missing ${start}`);
    return source.slice(from, source.indexOf(end, from));
  };
  const names = new Set();
  for (const m of block(core, 'const helperNameMap = {', '\n};').matchAll(/\[\w+\]:\s*`(\w+)`/g)) names.add(m[1]);
  for (const m of block(dom, 'registerRuntimeHelpers({', '});').matchAll(/\[\w+\]:\s*`(\w+)`/g)) names.add(m[1]);
  for (const m of sfc.matchAll(/ctx\.helper\(\s*[`"'](\w+)[`"']\s*\)/g)) names.add(m[1]);
  names.add(/const CSS_VARS_HELPER = `(\w+)`/.exec(sfc)[1]);
  return names;
}

// Hand-written: the 39 emitted, X-status helpers that are compilerInternal (generated code may import them).
// mergeProps is emitted too, but it is public (U13, P) and listed in compilerHelperExclusions instead.
const EXPECTED_FLAGGED = [
  'Fragment', 'camelize', 'createBlock', 'createCommentVNode', 'createElementBlock', 'createElementVNode',
  'createPropsRestProxy', 'createSlots', 'createStaticVNode', 'createTextVNode', 'createVNode', 'guardReactiveProps',
  'isMemoSame', 'mergeDefaults', 'mergeModels', 'normalizeClass', 'normalizeProps', 'normalizeStyle',
  'openBlock', 'renderList', 'renderSlot', 'resolveComponent', 'resolveDirective', 'resolveDynamicComponent',
  'setBlockTracking', 'toDisplayString', 'toHandlerKey', 'toHandlers', 'vModelCheckbox', 'vModelDynamic', 'vModelRadio',
  'vModelSelect', 'vModelText', 'vShow', 'withCtx', 'withDirectives', 'withKeys', 'withMemo', 'withModifiers',
];

test('the official compiler helper tables name 57 distinct helpers, all of them vue exports', async () => {
  const names = helperTableNames();
  const runtime = new Set(Object.keys(await import('vue')));
  assert.equal(names.size, 57);
  for (const name of names) assert.ok(runtime.has(name) || byName.has(name), name);
});

test('every compiler helper-table name that is a vue export is flagged compilerInternal or excluded with a reason', async () => {
  const runtime = new Set(Object.keys(await import('vue')));
  const exclusions = parseExportMap(exportMapText).compilerHelperExclusions;
  let checked = 0;
  for (const name of helperTableNames()) {
    if (!runtime.has(name)) continue;
    checked += 1;
    const flagged = byName.get(name).compilerInternal === true;
    const reason = exclusions[name];
    assert.ok(flagged || (typeof reason === 'string' && reason.trim() !== ''), `${name}: neither compilerInternal nor excluded with a reason`);
    assert.equal(flagged && reason !== undefined, false, `${name}: both flagged and excluded`);
  }
  assert.equal(checked, 57);
});

test('the compilerInternal set is exactly the 39 emitted X helpers, each with status X and a table entry', () => {
  const flagged = vueExports.exports.filter((entry) => entry.compilerInternal).map((entry) => entry.name).sort();
  assert.deepEqual(flagged, [...EXPECTED_FLAGGED].sort());
  const tables = helperTableNames();
  for (const name of flagged) {
    assert.equal(byName.get(name).status, 'X', name);
    assert.ok(tables.has(name), `${name} is not in the official helper tables`);
  }
});

test('compilerInternal helpers: user origin refuses them, compiler origin accepts them, on both targets', () => {
  for (const name of EXPECTED_FLAGGED) {
    for (const target of ['native', 'web']) {
      assert.equal(isSupported('vue', name, target), false, `${name} ${target} user`);
      assert.equal(isSupported('vue', name, target, { origin: 'user' }), false, `${name} ${target} user`);
      assert.equal(isSupported('vue', name, target, { origin: 'compiler' }), true, `${name} ${target} compiler`);
    }
  }
});

test('helpers the official compiler does not emit stay unsupported for both origins', () => {
  // h, isVNode and cloneVNode are not in this list: they are adapted public APIs (A11), supported for user code.
  const notEmitted = ['assertNumber', 'transformVNodeArgs', 'withScopeId', 'Comment', 'Static', 'Text', 'capitalize', 'pushScopeId', 'popScopeId', 'resolveFilter'];
  for (const name of notEmitted) {
    for (const target of ['native', 'web']) {
      assert.equal(isSupported('vue', name, target, { origin: 'compiler' }), false, `${name} ${target}`);
      assert.equal(isSupported('vue', name, target), false, `${name} ${target}`);
    }
  }
});

test('mergeProps is emitted by the compiler and is public: supported for user code and for generated code', () => {
  assert.equal(byName.get('mergeProps').compilerInternal, false);
  assert.equal(byName.get('mergeProps').status, 'P');
  assert.equal(byName.get('mergeProps').row, 'U13');
  assert.ok(parseExportMap(exportMapText).compilerHelperExclusions.mergeProps, 'mergeProps has an exclusion reason');
  for (const target of ['native', 'web']) {
    assert.equal(isSupported('vue', 'mergeProps', target), true, target);
    assert.equal(isSupported('vue', 'mergeProps', target, { origin: 'compiler' }), true, target);
  }
});

// Sample SFCs. Top-level await and <Suspense> are E in the profile and are left out on purpose.
const SAMPLES = {
  control: '<template><div><p v-if="a">{{ msg }}</p><p v-else-if="b">b</p><p v-else>c</p><li v-for="(x, i) in list" :key="i">{{ x }}</li></div></template>',
  slots: '<template><Child><template #header="{ item }"><span>{{ item }}</span></template></Child><div><slot name="head" :p="1" /><Comp><template #[dyn]><b>d</b></template></Comp></div></template>',
  dynamic: '<template><component :is="comp" /><component is="span" /></template>',
  transition: '<template><Transition><p v-if="a">t</p></Transition><TransitionGroup tag="ul"><li v-for="i in list" :key="i">{{ i }}</li></TransitionGroup></template>',
  keepAlive: '<template><KeepAlive><Comp /></KeepAlive><Teleport to="body"><p>x</p></Teleport></template>',
  models: '<template><input v-model="t" /><input type="checkbox" v-model="c" /><select v-model="s"><option>1</option></select><input type="radio" v-model="r" value="a" /><textarea v-model="ta" /><Comp v-model="m" /><input :type="kind" v-model="val" /></template>',
  directives: '<template><p v-show="v">s</p><input v-focus /></template>',
  events: '<template><button @click.stop.prevent="go" @keydown.enter="go">b</button><div v-on="handlers"></div><p @[evt]="go">d</p><p :[key].camel="v">x</p></template>',
  bindings: '<template><div :class="{ a: x }" :style="st" v-bind="attrs" :title="t">{{ msg }}</div><p v-html="h"></p><p v-text="t"></p><p v-once>o</p><ul><li v-for="item in items" :key="item.id" v-memo="[item.id]">{{ item.name }}</li></ul></template>',
  fragments: '<template><p>one</p><p>two</p><!-- note --></template>',
  staticTree: '<template><div>' + Array.from({ length: 24 }, (_, i) => `<p>line ${i}</p>`).join('') + '</div></template>',
  scriptPlain: "<script setup>import { ref, computed } from 'vue'; const count = ref(0); const d = computed(() => count.value * 2)</script><template><p>{{ d }}</p></template>",
  scriptProps: `<script setup lang="ts">const props = withDefaults(defineProps<{ a?: number, list?: string[] }>(), { a: 1, list: () => ['x'] })</script><template><p>{{ props.a }}</p></template>`,
  scriptRestProps: '<script setup lang="ts">const { a, ...rest } = defineProps<{ a: number, b?: string }>()</script><template><p>{{ a }}{{ rest.b }}</p></template>',
  scriptRuntimeDestructure: '<script setup>const { a = 1 } = defineProps({ a: Number })</script><template><p>{{ a }}</p></template>',
  scriptModels: '<script setup lang="ts">const props = defineProps({ a: Number }); const m = defineModel<string>()</script><template><p>{{ props.a }}{{ m }}</p></template>',
  scriptMacros: `<script setup lang="ts">const emit = defineEmits(['go']); defineExpose({ x: 1 }); defineOptions({ name: 'N', inheritAttrs: false }); defineSlots<{ default(p: {}): any }>()</script><template><p>e</p></template>`,
  cssVars: "<script setup>const color = 'red'</script><template><p>c</p></template><style>p { color: v-bind(color); }</style>",
  cssModule: `<script setup>const x = 1</script><template><p :class="$style.red">c</p></template><style module>.red { color: red; }</style>`,
};

// Names the official compiler emits for the samples, read from its output (import ... from "vue").
function emittedVueNames() {
  const sfc = requireFromRoot('@vue/compiler-sfc');
  const names = new Map();
  for (const [id, source] of Object.entries(SAMPLES)) {
    const { descriptor, errors } = sfc.parse(source, { filename: `${id}.vue` });
    assert.equal(errors.length, 0, `${id}: ${errors[0]}`);
    const code = descriptor.scriptSetup
      ? sfc.compileScript(descriptor, { id: 'x', inlineTemplate: true, isProd: false }).content
      : sfc.compileTemplate({ source: descriptor.template.content, filename: `${id}.vue`, id: 'x' }).code;
    for (const m of code.matchAll(/import\s*\{([^}]*)\}\s*from\s*["']vue["']/g)) {
      for (const part of m[1].split(',')) {
        const name = part.trim().split(/\s+as\s+/)[0].trim();
        if (name) names.set(name, (names.get(name) ?? 0) + 1);
      }
    }
  }
  return names;
}

test('the official compiler emits the expected helpers for the samples (observed, not assumed)', () => {
  const emitted = emittedVueNames();
  for (const name of ['openBlock', 'createElementBlock', 'toDisplayString', 'renderList', 'vModelText', 'vModelDynamic', 'withModifiers', 'toHandlers', 'mergeDefaults', 'mergeModels', 'createPropsRestProxy', 'createStaticVNode', 'isMemoSame', 'renderSlot', 'withCtx', 'camelize', 'toHandlerKey', 'setBlockTracking', 'normalizeClass', 'Fragment']) {
    assert.ok(emitted.has(name), `not emitted: ${name}`);
  }
});

test('generated code imports only names it may use: every emitted vue name is supported for the compiler origin on both targets', () => {
  for (const name of emittedVueNames().keys()) {
    for (const target of ['native', 'web']) {
      assert.equal(isSupported('vue', name, target, { origin: 'compiler' }), true, `${name} on ${target}`);
    }
  }
});

test('negative control: a helper-table name that is neither flagged nor excluded fails the audit', async () => {
  const rows = extractInventory(designText).rows;
  const map = parseExportMap(exportMapText);
  const { capitalize, ...withoutCapitalize } = map.compilerHelperExclusions;
  assert.ok(capitalize);
  await assert.rejects(auditVueExports({ rows, exportMap: { ...map, compilerHelperExclusions: withoutCapitalize } }), /capitalize/);
});

test('negative control: an exclusion without a reason fails the audit', async () => {
  const rows = extractInventory(designText).rows;
  const map = parseExportMap(exportMapText);
  const mutated = { ...map, compilerHelperExclusions: { ...map.compilerHelperExclusions, capitalize: '' } };
  await assert.rejects(auditVueExports({ rows, exportMap: mutated }), /capitalize" needs a reason/);
});

test('negative control: an exclusion that is not a compiler helper fails the audit', async () => {
  const rows = extractInventory(designText).rows;
  const map = parseExportMap(exportMapText);
  const mutated = { ...map, compilerHelperExclusions: { ...map.compilerHelperExclusions, ref: 'stale' } };
  await assert.rejects(auditVueExports({ rows, exportMap: mutated }), /"ref", which is not in a compiler helper table/);
});
