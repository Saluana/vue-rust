// @tier 1
// @covers U12
// A slot forwarded through two components arrives at the innermost <slot> with the same content.
import Page from './Page.vue'
export function run() {
  trace('html', testHost.mount(Page).html())
}
