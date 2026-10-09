import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {compile} from '../compiler/compile.mjs';
const component=(script,template='<p>{{ count }}</p>')=>`<script setup>${script}</script><template>${template}</template>`;
const base="import {ref, computed} from 'vue'; const count = ref(0);";
test('real Composition API SFC emits native state, computed, events, conditional and input',()=>{
 const rust=compile(readFileSync(new URL('../examples/Counter.vue',import.meta.url),'utf8'));
 for(const expected of ['pub count: f64','pub name: String','pub fn doubled','self.count += 1.0','text_edit_singleline','if (self.count >','self.increment();']) assert.ok(rust.includes(expected),expected);
 assert.ok(!rust.includes('eval('));
});
test('template auto unwrap and script .value',()=>assert.match(compile(component(base+'const double = computed(() => count.value * 2);','<p>{{ double }}</p>')),/self.double\(\)/));
test('inline update',()=>assert.match(compile(component(base,'<button @click="count++">Add</button>')),/self.count \+= 1.0/));
test('boolean conditional',()=>assert.match(compile(component("import {ref} from 'vue'; const visible=ref(true);",'<p v-if="visible">Hi</p>')),/if self.visible/));
test('forward computed dependency',()=>assert.match(compile(component(base+'const a=computed(() => b.value * 2);const b=computed(() => count.value + 1);','<p>{{ a }}</p>')),/self.b\(\)/));
for(const [name,source,pattern] of [
 ['Options API','<script>export default {}</script><template><p/></template>',/script setup/],
 ['CSS',component(base)+'<style>p{color:red}</style>',/CSS/],
 ['unsupported directive',component(base,'<p v-for="x in count">x</p>'),/directive/],
 ['unsupported element',component(base,'<video/>'),/element/],
 ['unsupported import',component("import thing from 'anything';"),/Only ref/],
 ['unknown template binding',component(base,'<p>{{ missing }}</p>'),/Unknown/],
 ['non literal ref',component(base+'const x=ref(count.value);'),/initializers/],
 ['computed mutation',component(base+'const x=computed(() => count.value);','<button @click="x++">x</button>'),/Only refs/],
 ['type mismatch',component(base,'<button @click="count = true">x</button>'),/mismatch/],
 ['cycles',component(base+'const a=computed(() => b.value);const b=computed(() => a.value);'),/Cyclic/],
 ['async handler',component(base+'async function inc(){count.value++}'),/synchronous/],
 ['unknown attribute',component(base,'<p class="hi">Hi</p>'),/attribute/],
 ['event on unsupported node',component(base,'<p @click="count++">Hi</p>'),/invalid/],
 ['number input',component(base,'<input v-model="count"/>'),/string ref/],
 ['JavaScript loose coercion',component(base,'<p>{{ count == 0 }}</p>'),/Unsupported/],
 ['arbitrary JS',component(base+'console.log(count);'),/Unsupported/],
 ['nonboolean v-if',component(base,'<p v-if="count">x</p>'),/boolean/],
 ['modifiers',component(base,'<button @click.stop="count++">x</button>'),/directive/],
]) test(`rejects ${name} instead of silently changing behavior`,()=>assert.throws(()=>compile(source),pattern));
test('string escaping preserves Rust source boundaries',()=>{
 const rust=compile(component("import {ref} from 'vue';const count=ref('quote \\\" and \\b and \\\\ path');"));
 assert.ok(rust.includes('\\u{8}'));
 assert.ok(rust.includes('quote \\\"'));
});
test('unimplemented type annotations are rejected',()=>assert.throws(()=>compile(component("import {ref} from 'vue';const count=ref<string>(1);")),/TypeScript/));
