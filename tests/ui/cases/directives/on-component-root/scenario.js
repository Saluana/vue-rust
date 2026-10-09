// @tier 2
// @covers U26,U13
// A directive on a component element is applied to the component's root element.
import Page from './Page.vue'
export function run() {
  trace('html', testHost.mount(Page, { log: (v) => trace('log', v) }).html())
}
