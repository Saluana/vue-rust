// @tier 2
// @covers U06
// Key modifiers: .enter matches the key alone; .ctrl.enter.exact needs exactly Control, so Control+Shift+Enter fires only the plain handler.
import Keys from './Keys.vue'
export function run() {
  const m = testHost.mount(Keys, { onKey: (v) => trace('key', v) })
  const k = testHost.find(m, '#k')
  testHost.keydown(k, 'Enter')
  testHost.keydown(k, 'Enter', { ctrlKey: true })
  testHost.keydown(k, 'Enter', { ctrlKey: true, shiftKey: true })
  testHost.keydown(k, 'Escape')
  testHost.keydown(k, 'a')
  trace('count', testHost.findAll(m, 'input').length)
}
