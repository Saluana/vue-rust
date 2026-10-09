// @tier 1
// @covers V27,U18
// useTemplateRef resolves to the element, and a focus action from mounted fires the element's focus listener.
import Input from './Input.vue'
export function run() {
  testHost.mount(Input, { log: (v) => trace('log', v) })
}
