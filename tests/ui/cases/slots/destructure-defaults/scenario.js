// @tier 1
// @covers U12
// A destructuring default in a scoped slot applies when the child passes no such prop.
import Page from './Page.vue'
export function run() {
  trace('html', testHost.mount(Page).html())
}
