// @tier 2
// @covers U06
// Button filters: .left listens to click, .middle to mouseup and .right to contextmenu (the compiler maps them), each checked against the button.
import Mouse from './Mouse.vue'
export function run() {
  const m = testHost.mount(Mouse, { onB: (v) => trace('b', v) })
  const b = testHost.find(m, '#m')
  testHost.dispatch(b, 'click', { button: 0 })
  testHost.dispatch(b, 'click', { button: 1 })
  testHost.dispatch(b, 'mouseup', { button: 1 })
  testHost.dispatch(b, 'contextmenu', { button: 2 })
}
