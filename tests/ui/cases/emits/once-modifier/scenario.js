// @tier 2
// @covers U11,U06
// The .once modifier on a component listener runs its handler for the first emit only.
import Parent from './Parent.vue'
export function run() {
  const m = testHost.mount(Parent, { log: (v) => trace('log', v) })
  const b = testHost.find(m, '#b')
  testHost.click(b)
  testHost.click(b)
  testHost.click(b)
}
