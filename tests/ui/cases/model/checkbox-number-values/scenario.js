// @tier 2
// @covers U09
// Checkbox true-value and false-value bind any JavaScript values, here the numbers 1 and 0.
import Chk from './Chk.vue'
export async function run() {
  const m = testHost.mount(Chk)
  testHost.check(testHost.find(m, '#c'), true)
  await testHost.tick()
  trace('out', testHost.find(m, '#out').text())
  testHost.check(testHost.find(m, '#c'), false)
  await testHost.tick()
  trace('again', testHost.find(m, '#out').text())
}
