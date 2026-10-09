// @tier 2
// @covers U21,V22
// KeepAlive include keeps only the named component: B is destroyed when it is left, and A is deactivated instead.
import A from './A.vue'
import B from './B.vue'
import Inc from './Inc.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const m = testHost.mount(Inc, { current: A, log })
  m.setProps({ current: B, log })
  await testHost.tick()
  m.setProps({ current: A, log })
  await testHost.tick()
  trace('html', m.html())
}
