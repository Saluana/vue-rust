// @tier 1
// @covers U11
// A failing validator or a missing required prop only warns in development; the production build renders the value it was given.
import Req from './Req.vue'
export function run() {
  trace('invalid', testHost.find(testHost.mount(Req, { id: 'a', size: 'x' }), '#p').text())
  trace('missing', testHost.find(testHost.mount(Req), '#p').text())
}
