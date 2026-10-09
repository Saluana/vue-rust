// @tier 1
// @covers V24
// A provider's value reaches only its own subtree; a sibling outside the provider injects the default.
import Page from './Page.vue'
export function run() {
  trace('leaves', testHost.findAll(testHost.mount(Page), '.leaf').map((h) => h.text()))
}
