// @tier 2
// @covers V20,A06
// An error thrown by an event handler goes to the app errorHandler with the native event info, and later events still run.
import Btn from './Btn.vue'
export function run() {
  const m = testHost.mount(Btn, {}, { errorHandler: (err, inst, info) => trace('app', err.message, info) })
  testHost.click(testHost.find(m, '#b'))
  testHost.click(testHost.find(m, '#b'))
  trace('done')
}
