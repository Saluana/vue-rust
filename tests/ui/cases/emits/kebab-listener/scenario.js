// @tier 1
// @covers U11
// A camelCase emit name is reached from a kebab-case listener in the parent template.
import Parent from './Parent.vue'
export function run() {
  const m = testHost.mount(Parent, { log: (...v) => trace('log', ...v) })
  testHost.click(testHost.find(m, '#k'))
}
