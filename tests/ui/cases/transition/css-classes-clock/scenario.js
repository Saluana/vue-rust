// @tier 3
// @covers U23
// With CSS transitions enabled and no stylesheet, the enter classes come and go on animation frames of the fake clock.
import Css from './Css.vue'
export async function run() {
  const m = testHost.mount(Css, { show: false })
  m.setProps({ show: true })
  await testHost.tick()
  trace('0', m.html())
  await testHost.timers.advance(16)
  await testHost.tick()
  trace('16', m.html())
  await testHost.timers.advance(64)
  await testHost.tick()
  trace('80', m.html())
}
