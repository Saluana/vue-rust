// @tier 1
// @covers U18
// A ref on a v-for element collects one element per item, in order.
import Items from './Items.vue'
export function run() {
  testHost.mount(Items, { log: (v) => trace('log', v) })
}
