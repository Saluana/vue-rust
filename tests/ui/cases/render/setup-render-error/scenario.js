// @tier 1
// @covers A10,A06
// An error thrown by a render function returned from setup goes to the app errorHandler, and the component renders an empty comment in its place.
import Broken from './Broken.vue'
export function run() {
  const m = testHost.mount(Broken, {}, { errorHandler: (err, inst, info) => trace('app', err.message, info) })
  trace('html', m.html())
}
