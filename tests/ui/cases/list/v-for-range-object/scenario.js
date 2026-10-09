// @tier 1
// @covers U16
// v-for over an integer range, an object (value, key, index) and a string, rendered in that order.
import Ranges from './Ranges.vue'
export function run() {
  const m = testHost.mount(Ranges, { obj: { x: 1, y: 'two' }, n: 3, text: 'ab' })
  trace('html', m.html())
}
