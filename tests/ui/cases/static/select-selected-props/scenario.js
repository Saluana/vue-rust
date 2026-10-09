// @tier 1
// @covers U09
// The first option of a single select is selected without a selected attribute, which the snapshot reports as a selected prop.
import Sel from './Sel.vue'
export function run() {
  const m = testHost.mount(Sel)
  trace('snapshot', m.snapshot())
  trace('values', testHost.find(m, '#s1').prop('value'), testHost.find(m, '#s2').prop('value'))
}
