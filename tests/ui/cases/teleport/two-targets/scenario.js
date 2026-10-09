// @tier 1
// @covers U22
// Two Teleports send their content to two different containers.
import Page from './Page.vue'
export function run() {
  const a = testHost.container('a')
  const b = testHost.container('b')
  testHost.mount(Page)
  trace('a', a.html(), 'b', b.html())
}
