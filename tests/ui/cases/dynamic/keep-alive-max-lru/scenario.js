// @tier 2
// @covers U21
// With max 1, KeepAlive keeps one cached instance: each new view evicts the previous one (A is unmounted when B arrives), so A starts over.
import A from './A.vue'
import B from './B.vue'
import C from './C.vue'
import Lru from './Lru.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const m = testHost.mount(Lru, { current: A, log })
  testHost.click(testHost.find(m, '#btn'))
  m.setProps({ current: B, log })
  await testHost.tick()
  m.setProps({ current: C, log })
  await testHost.tick()
  m.setProps({ current: A, log })
  await testHost.tick()
  trace('html', m.html())
}
