// @tier 1
// @covers U13,U11
// A listener for an event the component does not declare falls through to its root element as a native listener.
import Undecl from './Undecl.vue'
export function run() {
  const m = testHost.mount(Undecl, { onPing: () => trace('ping') })
  testHost.dispatch(testHost.find(m, '#b'), 'ping')
  testHost.dispatch(testHost.find(m, '#b'), 'pong')
  trace('count', testHost.findAll(m, 'button').length)
}
