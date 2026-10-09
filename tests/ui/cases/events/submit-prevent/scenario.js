// @tier 2
// @covers U06
// A submit listener with .prevent runs once for a dispatched submit event.
import Form from './Form.vue'
export function run() {
  const m = testHost.mount(Form, { onEv: (v) => trace('ev', v) })
  testHost.dispatch(testHost.find(m, '#f'), 'submit')
}
