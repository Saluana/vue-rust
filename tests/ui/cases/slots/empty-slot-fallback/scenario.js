// @tier 1
// @covers U12
// A named slot whose parent content is empty renders the child's fallback.
import Page from './Page.vue'
export function run() {
  trace('html', testHost.mount(Page).html())
}
