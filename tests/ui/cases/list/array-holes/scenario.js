// @tier 1
// @covers U16
// An array with holes: v-for visits every index, so a hole renders as an undefined item, which prints as nothing.
import Items from './Items.vue'
export function run() {
  trace('html', testHost.mount(Items, { items: [{ id: 1, tag: 'a' }, , { id: 3, tag: 'c' }] }).html())
}
