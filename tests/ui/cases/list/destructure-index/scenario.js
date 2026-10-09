// @tier 1
// @covers U16
// v-for with a destructured item and an index alias: the index counts from 0 in array order.
import Items from './Items.vue'
export function run() {
  trace('html', testHost.mount(Items, { items: [{ id: 7, tag: 'x' }, { id: 9, tag: 'y' }] }).html())
}
