// @tier 2
// @covers U11,U09
// Two v-model arguments on one component update two independent parent refs.
import Parent from './Parent.vue'
export async function run() {
  const m = testHost.mount(Parent)
  testHost.input(testHost.find(m, '#y'), 'second-changed')
  await testHost.tick()
  trace('out', testHost.find(m, '#out').text())
}
