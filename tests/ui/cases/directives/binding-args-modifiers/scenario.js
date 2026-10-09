// @tier 1
// @covers U26
// A directive receives its argument, its modifiers and its value in the binding object.
import Demo from './Demo.vue'
export function run() {
  testHost.mount(Demo, { log: (v) => trace('log', v) })
}
