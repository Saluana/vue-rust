// @tier 2
// @covers U05,U06
// Click modifiers: .stop ends bubbling, .self ignores clicks from children, .once runs a single time; bubbling reaches the outer handler.
import Clicks from './Clicks.vue'
export async function run() {
  const m = testHost.mount(Clicks, { onHit: (v) => trace('hit', v) })
  testHost.click(testHost.find(m, '#inner'))
  testHost.click(testHost.find(m, '#self'))
  testHost.click(testHost.find(m, '#outer'))
  testHost.click(testHost.find(m, '#link'))
  testHost.click(testHost.find(m, '#once'))
  testHost.click(testHost.find(m, '#once'))
  await testHost.tick()
  trace('done', testHost.find(m, '#once').exists())
}
