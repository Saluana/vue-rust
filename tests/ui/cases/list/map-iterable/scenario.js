// @tier 1
// @covers U16
// v-for over a Map iterates its entries in insertion order.
import Entries from './Entries.vue'
export function run() {
  trace('html', testHost.mount(Entries, { map: new Map([['b', 2], ['a', 1]]) }).html())
}
