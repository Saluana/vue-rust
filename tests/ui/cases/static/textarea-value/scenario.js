// @tier 1
// @covers U03
// A bound textarea value is a dirty value property; static text content is the default value and gives no props.
import Area from './Area.vue'
export function run() {
  const m = testHost.mount(Area)
  trace('snapshot', m.snapshot())
  trace('value', testHost.find(m, '#a').prop('value'), testHost.find(m, '#b').prop('value'))
}
