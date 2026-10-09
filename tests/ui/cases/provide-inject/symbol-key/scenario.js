// @tier 1
// @covers V24
// Symbol keys identify provisions exactly: the value comes back through the same symbol, and a string key does not match it.
import Provider from './Provider.vue'
export function run() {
  const KEY = Symbol('k')
  trace('text', testHost.find(testHost.mount(Provider, { k: KEY }), '#p').text())
}
