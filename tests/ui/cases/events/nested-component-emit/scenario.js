// @tier 2
// @covers U05,U11
// A component emit runs inside the DOM click listener of its child; the DOM event then bubbles to the outer element.
import Outer from './Outer.vue'
export function run() {
  const m = testHost.mount(Outer, { onSeen: (v) => trace('seen', v) })
  testHost.click(testHost.find(m, '#go'))
}
