// @tier 1
// @covers A10,A11
// A component whose setup returns a render function renders the VNode tree built by h: class and style are normalized, attributes are set, and array and text children become DOM nodes in order.
import Tree from './Tree.vue'
export function run() {
  const m = testHost.mount(Tree)
  trace('html', m.html())
  trace('snapshot', m.snapshot())
  trace('heading', testHost.find(m, 'h2').attr('title'), testHost.find(m, 'section').attr('class'))
}
