// @tier 1
// @covers V19
// Unmounting runs beforeUnmount on the parent and then the child, and unmounted in the reverse order.
import Parent from './Parent.vue'
export function run() {
  const m = testHost.mount(Parent, { log: (v) => trace('log', v), n: 1 })
  trace('unmounting')
  m.unmount()
  trace('html', m.html())
}
