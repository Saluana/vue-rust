// @tier 1
// @covers U11
// A declared event passes all of its payload arguments to the listener, in order.
import Em from './Em.vue'
export function run() {
  const m = testHost.mount(Em, { onSave: (...v) => trace('save', ...v) })
  testHost.click(testHost.find(m, '#b'))
}
