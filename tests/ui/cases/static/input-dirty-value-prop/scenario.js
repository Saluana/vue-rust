// @tier 2
// @covers U03,U07
// Typing without a model changes the value property only: the snapshot lists value as a prop and the attribute stays as bound.
import Dirty from './Dirty.vue'
export async function run() {
  const m = testHost.mount(Dirty, { on: false })
  testHost.input(testHost.find(m, '#i'), 'typed')
  testHost.input(testHost.find(m, '#j'), 'x')
  await testHost.tick()
  trace('snapshot', m.snapshot())
  m.setProps({ on: true })
  await testHost.tick()
  trace('checked', testHost.find(m, '#k').prop('checked'), testHost.find(m, '#k').attr('checked'))
}
