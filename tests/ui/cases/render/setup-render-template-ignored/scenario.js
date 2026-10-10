// @tier 1
// @covers A10,U30
// When setup returns a render function, the render function is the component's render: the SFC template is compiled but not rendered.
import Both from './Both.vue'
export function run() {
  const m = testHost.mount(Both)
  trace('html', m.html())
  trace('templates', testHost.findAll(m, '#from-template').length, testHost.findAll(m, '#from-setup').length)
}
