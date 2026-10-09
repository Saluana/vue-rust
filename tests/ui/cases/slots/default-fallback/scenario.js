// @tier 1
// @covers U12
// Default slot content replaces the fallback only when the parent provides content.
import Page from './Page.vue'
export function run() {
  trace('html', testHost.mount(Page).html())
}
