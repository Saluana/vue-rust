// @tier 1
// @covers U12
// Slot content is compiled in the parent's scope: it reads the parent's msg, not the child's.
import Page from './Page.vue'
export function run() {
  trace('html', testHost.mount(Page).html())
}
