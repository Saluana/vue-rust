// @tier 2
// @covers U14,V18
// Hiding a child with v-if destroys its state: a remounted child starts from its initial value.
import Parent from './Parent.vue'
export async function run() {
  const m = testHost.mount(Parent, { show: true })
  testHost.click(testHost.find(m, '#c'))
  testHost.click(testHost.find(m, '#c'))
  await testHost.tick()
  trace('clicked', testHost.find(m, '#c').text())
  m.setProps({ show: false })
  await testHost.tick()
  m.setProps({ show: true })
  await testHost.tick()
  trace('remounted', testHost.find(m, '#c').text())
}
