// @tier 1
// @covers V20,A06
// A render error goes to the app errorHandler option, and the component renders an empty comment in its place.
import Broken from './Broken.vue'
export function run() {
  const m = testHost.mount(Broken, {}, { errorHandler: (err, inst, info) => trace('app', err.message, info) })
  trace('html', m.html())
}
