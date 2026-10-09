// @tier 1
// @covers U12
// Scoped slot props carry the child's item and index into the parent's slot content.
import Page from './Page.vue'
export function run() {
  trace('html', testHost.mount(Page, { items: ['a', 'b'] }).html())
}
