// @tier 1
// @covers V24
// inject falls back to its default value, or to a factory result when the third argument is true.
import Leaf from './Leaf.vue'
export function run() {
  trace('text', testHost.find(testHost.mount(Leaf), '#p').text())
}
