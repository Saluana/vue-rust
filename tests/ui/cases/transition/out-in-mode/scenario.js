// @tier 3
// @covers U23
// mode out-in: the old element leaves completely before the new one enters; the hooks are timed on the fake clock.
import Swap from './Swap.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const schedule = (ms, done) => testHost.timers.setTimeout(done, ms)
  const m = testHost.mount(Swap, { name: 'a', log, schedule })
  m.setProps({ name: 'b', log, schedule })
  await testHost.tick()
  trace('leaving', m.html())
  await testHost.timers.advance(20)
  await testHost.tick()
  trace('entering', m.html())
  await testHost.timers.advance(20)
  await testHost.tick()
  trace('done', m.html())
}
