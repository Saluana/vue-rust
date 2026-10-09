// @tier 1
// @covers V18
// Sibling components mount in template order: each beforeMount runs before its mounted hook, and mounted hooks run in tree order.
import Pair from './Pair.vue'
export function run() {
  testHost.mount(Pair, { log: (v) => trace('log', v) })
}
