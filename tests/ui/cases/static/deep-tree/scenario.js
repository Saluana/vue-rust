// @tier 1
// @covers U30,U31
// A nested tree of semantic elements: void elements, inline text and whitespace inside pre are kept as written.
import Tree from './Tree.vue'
export function run() {
  const m = testHost.mount(Tree)
  trace('html', m.html())
  trace('count', testHost.findAll(m, 'li').length, testHost.find(m, 'img').attr('alt'))
}
