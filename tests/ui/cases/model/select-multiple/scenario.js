// @tier 2
// @covers U09
// A multiple select binds an array of the selected values, in option order.
import Multi from './Multi.vue'
export async function run() {
  const m = testHost.mount(Multi)
  testHost.select(testHost.find(m, '#s'), ['c', 'a'])
  await testHost.tick()
  trace('out', testHost.find(m, '#out').text())
}
