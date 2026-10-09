// @tier 1
// @covers U30
// An empty template renders an empty comment node, so the component still has a place in the tree.
import Empty from './Empty.vue'
export function run() {
  const m = testHost.mount(Empty)
  trace('html', m.html())
  trace('snapshot', m.snapshot())
}
