// @tier 2
// @covers U19,V18
// <component :is> swaps the rendered component; switching back creates a fresh instance whose state starts over.
import A from './A.vue'
import B from './B.vue'
import Host from './Host.vue'
export async function run() {
  const m = testHost.mount(Host, { current: A })
  testHost.click(testHost.find(m, '#btn'))
  testHost.click(testHost.find(m, '#btn'))
  await testHost.tick()
  trace('a', m.html())
  m.setProps({ current: B })
  await testHost.tick()
  trace('b', m.html())
  m.setProps({ current: A })
  await testHost.tick()
  trace('again', m.html())
}
