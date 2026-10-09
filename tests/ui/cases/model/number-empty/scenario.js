// @tier 2
// @covers U08
// .number keeps an empty string as it is and converts numeric text; text that is not a number stays a string.
import Num from './Num.vue'
export async function run() {
  const m = testHost.mount(Num)
  const n = testHost.find(m, '#n')
  testHost.input(n, '')
  await testHost.tick()
  trace('empty', testHost.find(m, '#out').text())
  testHost.input(n, '3')
  await testHost.tick()
  trace('three', testHost.find(m, '#out').text())
  testHost.input(n, '2x')
  await testHost.tick()
  trace('text', testHost.find(m, '#out').text())
}
