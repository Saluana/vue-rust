// @tier 3
// @covers U23
// Showing the element again while it leaves cancels the leave: leave-cancelled runs and the element enters again.
import Cancel from './Cancel.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const schedule = (ms, done) => testHost.timers.setTimeout(done, ms)
  const m = testHost.mount(Cancel, { show: true, log, schedule })
  m.setProps({ show: false, log, schedule })
  await testHost.tick()
  await testHost.timers.advance(20)
  m.setProps({ show: true, log, schedule })
  await testHost.tick()
  await testHost.timers.advance(60)
  await testHost.tick()
  trace('html', m.html())
}
