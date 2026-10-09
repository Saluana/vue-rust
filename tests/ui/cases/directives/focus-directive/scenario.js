// @tier 2
// @covers U26
// A directive that focuses its element in mounted makes the element fire its focus listener during mount.
import Focus from './Focus.vue'
export function run() {
  testHost.mount(Focus, { log: (v) => trace('log', v) })
}
