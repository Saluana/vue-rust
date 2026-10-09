// @tier 2
// @covers U05
// Moving focus from one field to another fires blur on the first, then focus on the second.
import Focus from './Focus.vue'
export function run() {
  const m = testHost.mount(Focus, { onEv: (v) => trace('ev', v) })
  testHost.focus(testHost.find(m, '#a'))
  testHost.focus(testHost.find(m, '#b'))
}
