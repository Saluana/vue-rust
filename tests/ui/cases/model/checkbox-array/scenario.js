// @tier 2
// @covers U09
// v-model on checkboxes: a bound array keeps the order in which boxes were checked; a plain box binds a boolean; true-value and false-value bind strings.
// Each user action is followed by a tick, as in a browser where the DOM updates between two clicks.
import Checks from './Checks.vue'
export async function run() {
  const m = testHost.mount(Checks)
  testHost.check(testHost.find(m, '#a'), true)
  await testHost.tick()
  testHost.check(testHost.find(m, '#b'), false)
  await testHost.tick()
  testHost.check(testHost.find(m, '#c'), true)
  await testHost.tick()
  testHost.check(testHost.find(m, '#f'), true)
  testHost.check(testHost.find(m, '#tv'), true)
  await testHost.tick()
  trace('state', m.html())
  trace('checked', testHost.find(m, '#a').prop('checked'), testHost.find(m, '#b').prop('checked'))
}
