// @tier 1
// @covers V18
// Mount hooks run child first (beforeMount, mounted), then the parent's mounted hook.
import Parent from './Parent.vue'
export function run() {
  testHost.mount(Parent, { log: (v) => trace('log', v), n: 1 })
}
