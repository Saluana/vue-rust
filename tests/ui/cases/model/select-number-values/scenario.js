// @tier 2
// @covers U09
// Options bound with :value keep their JavaScript value: the model receives the number 2, not the string "2".
import NumSel from './NumSel.vue'
export async function run() {
  const m = testHost.mount(NumSel)
  testHost.select(testHost.find(m, '#s'), '2')
  await testHost.tick()
  trace('out', testHost.find(m, '#out').text())
}
