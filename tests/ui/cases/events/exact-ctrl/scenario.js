// @tier 2
// @covers U06
// .exact requires that no other modifier key is held: Control alone fires, Control with Shift does not.
import Exact from './Exact.vue'
export function run() {
  const m = testHost.mount(Exact, { onX: (v) => trace('x', v) })
  const x = testHost.find(m, '#x')
  testHost.click(x)
  testHost.dispatch(x, 'click', { ctrlKey: true })
  testHost.dispatch(x, 'click', { ctrlKey: true, shiftKey: true })
}
