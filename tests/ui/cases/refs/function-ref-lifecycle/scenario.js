// @tier 2
// @covers U18
// A function ref is called with the element on mount and with null when the element is removed.
import Fn from './Fn.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const m = testHost.mount(Fn, { log, show: true })
  m.setProps({ log, show: false })
  await testHost.tick()
  trace('html', m.html())
}
