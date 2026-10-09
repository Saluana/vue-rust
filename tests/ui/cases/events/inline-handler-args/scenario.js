// @tier 2
// @covers U05
// Inline handlers receive $event; a call in a method receives the same event object, read for its type and button.
import Args from './Args.vue'
export function run() {
  const m = testHost.mount(Args, { onPick: (...v) => trace('pick', ...v) })
  testHost.click(testHost.find(m, '#a'))
  testHost.click(testHost.find(m, '#b'))
}
