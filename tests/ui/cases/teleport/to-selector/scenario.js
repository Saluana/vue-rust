// @tier 1
// @covers U22
// Teleport moves its content into the target container; the component's own html keeps only the local content.
import Page from './Page.vue'
export function run() {
  const overlay = testHost.container('overlay')
  const m = testHost.mount(Page)
  trace('local', m.html())
  trace('overlay', overlay.html())
}
