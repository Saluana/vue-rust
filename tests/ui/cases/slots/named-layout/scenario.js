// @tier 1
// @covers U12
// Named slots take <template #name> content; the default slot takes the remaining children.
import Page from './Page.vue'
export function run() {
  trace('html', testHost.mount(Page).html())
}
