// @tier 1
// @covers U01,U30
// Text interpolation and element nesting: the canonical snapshot and the html of a mounted static template.
import Greeting from './Greeting.vue'
export function run() {
  const m = testHost.mount(Greeting, { name: 'world' })
  trace('html', m.html())
  trace('snapshot', m.snapshot())
  m.unmount()
  trace('after-unmount', m.snapshot())
}
