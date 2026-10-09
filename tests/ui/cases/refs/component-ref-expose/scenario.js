// @tier 1
// @covers U18,V27
// A component template ref exposes only what defineExpose lists; the parent calls the exposed function on mount.
import Parent from './Parent.vue'
export function run() {
  testHost.mount(Parent, { log: (v) => trace('log', v) })
}
