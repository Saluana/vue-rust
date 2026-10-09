// @tier 1
// @covers V24,U12
// Slot content is rendered by the child that receives it, so it injects from that child's providers, not from the page that wrote it.
import Page from './Page.vue'
export function run() {
  trace('html', testHost.mount(Page).html())
}
