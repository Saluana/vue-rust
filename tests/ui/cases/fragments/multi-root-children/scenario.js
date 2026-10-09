// @tier 1
// @covers U30
// A component with two roots contributes both nodes at its position among its siblings.
import Page from './Page.vue'
export function run() {
  trace('html', testHost.mount(Page).html())
}
