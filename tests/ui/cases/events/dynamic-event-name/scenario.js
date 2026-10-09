// @tier 2
// @covers U05
// A dynamic event name re-binds the listener when the name changes: the old event no longer reaches the handler.
import Dyn from './Dyn.vue'
export async function run() {
  const m = testHost.mount(Dyn, { evt: 'focus', onFired: (v) => trace('fired', v) })
  testHost.focus(testHost.find(m, '#d'))
  m.setProps({ evt: 'keydown', onFired: (v) => trace('fired', v) })
  await testHost.tick()
  testHost.focus(testHost.find(m, '#d'))
  testHost.keydown(testHost.find(m, '#d'), 'a')
}
