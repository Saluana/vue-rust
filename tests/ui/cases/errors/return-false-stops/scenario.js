// @tier 2
// @covers V20
// Returning false from onErrorCaptured stops the error: the app-level handler is not called.
import Parent from './Parent.vue'
export function run() {
  const m = testHost.mount(Parent, { log: (v) => trace('log', v) }, { errorHandler: (err, inst, info) => trace('app', err.message, info) })
  testHost.click(testHost.find(m, '#b'))
  trace('survived')
}
