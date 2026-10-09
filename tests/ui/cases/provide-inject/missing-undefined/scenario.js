// @tier 1
// @covers V24
// Without a default, a missing key injects undefined (production builds print no warning).
import Leaf from './Leaf.vue'
export function run() {
  trace('text', testHost.find(testHost.mount(Leaf), '#p').text())
}
