// @tier 2
// @covers U05
// Capture listeners run before the target, and bubble listeners after it: parent-capture, child, parent-bubble.
import Cap from './Cap.vue'
export function run() {
  const m = testHost.mount(Cap, { onLog: (v) => trace('log', v) })
  testHost.click(testHost.find(m, '#c'))
}
