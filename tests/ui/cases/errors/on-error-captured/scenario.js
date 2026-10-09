// @tier 1
// @covers V20
// An error thrown by a child's setup reaches the parent's onErrorCaptured with the setup info string.
import Parent from './Parent.vue'
export function run() {
  trace('html', testHost.mount(Parent, { log: (v) => trace('log', v) }).html())
}
