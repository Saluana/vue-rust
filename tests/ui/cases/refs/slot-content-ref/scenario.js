// @tier 1
// @covers U18,U12
// A template ref written inside slot content belongs to the component whose template contains it, and it is set after mount.
import Page from './Page.vue'
export function run() {
  testHost.mount(Page, { log: (v) => trace('log', v) })
}
