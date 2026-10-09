// @tier 1
// @covers U30,U01
// A component whose template is only text renders a text node at its position.
import Txt from './Txt.vue'
export function run() {
  const m = testHost.mount(Txt, { msg: 'hi' })
  trace('html', m.html())
  trace('snapshot', m.snapshot())
}
