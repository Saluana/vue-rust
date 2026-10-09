// @tier 1
// @covers V18,V29
// Setup code runs before the beforeMount hook, which runs before mounted.
import Setup from './Setup.vue'
export function run() {
  testHost.mount(Setup, { log: (v) => trace('log', v) })
}
