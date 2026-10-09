// @tier 1
// @covers U12
// The child passes props to the slot content; without parent content the child's own fallback renders instead.
import Page from './Page.vue'
export function run() {
  trace('html', testHost.mount(Page).html())
}
