// @tier 3
// @covers V20,A06
// A rejected promise returned by an event handler is reported to the app errorHandler after the microtask.
import Btn from './Btn.vue'
export async function run() {
  const m = testHost.mount(Btn, {}, { errorHandler: (err, inst, info) => trace('app', err.message, info) })
  testHost.click(testHost.find(m, '#b'))
  trace('sync-done')
  await testHost.tick()
  trace('after-tick')
}
