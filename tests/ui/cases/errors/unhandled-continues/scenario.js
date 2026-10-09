// @tier 2
// @covers V20,A06
// Without any handler, an error in an event handler is logged and the component keeps working.
import Btn from './Btn.vue'
export async function run() {
  const m = testHost.mount(Btn)
  testHost.click(testHost.find(m, '#b'))
  testHost.click(testHost.find(m, '#b'))
  await testHost.tick()
  trace('text', testHost.find(m, '#b').text())
}
