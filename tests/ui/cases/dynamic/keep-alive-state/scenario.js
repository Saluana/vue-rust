// @tier 2
// @covers U21,V22
// KeepAlive keeps a deactivated component's state: the counter is still 2 when the component is shown again.
import A from './A.vue'
import B from './B.vue'
import Keep from './Keep.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const m = testHost.mount(Keep, { current: A, log })
  testHost.click(testHost.find(m, '#btn'))
  testHost.click(testHost.find(m, '#btn'))
  await testHost.tick()
  m.setProps({ current: B, log })
  await testHost.tick()
  m.setProps({ current: A, log })
  await testHost.tick()
  trace('html', m.html())
}
