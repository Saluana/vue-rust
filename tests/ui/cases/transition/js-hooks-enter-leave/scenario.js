// @tier 3
// @covers U23,V18
// A Transition with JS hooks and no CSS runs before-enter, enter, after-enter, and the leave hooks in order; the timers are the fake clock.
import Fade from './Fade.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const schedule = (ms, done) => testHost.timers.setTimeout(done, ms)
  const m = testHost.mount(Fade, { show: false, log, schedule })
  m.setProps({ show: true, log, schedule })
  await testHost.tick()
  trace('mid-enter', m.html())
  await testHost.timers.advance(30)
  await testHost.tick()
  m.setProps({ show: false, log, schedule })
  await testHost.tick()
  trace('mid-leave', m.html())
  await testHost.timers.advance(40)
  await testHost.tick()
  trace('end', m.html())
}
