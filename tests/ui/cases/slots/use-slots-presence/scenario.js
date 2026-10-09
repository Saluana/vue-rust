// @tier 1
// @covers U12,V25
// useSlots reports which slots the parent passed; a named slot is present only when it is provided.
import Page from './Page.vue'
export function run() {
  trace('html', testHost.mount(Page).html())
}
