// @tier 2
// @covers U22,U05
// A teleported button keeps its listeners, and its emit reaches the component that owns the Teleport.
import Page from './Page.vue'
export function run() {
  const overlay = testHost.container('overlay')
  testHost.mount(Page, { onHit: (v) => trace('hit', v) })
  testHost.click(testHost.find(overlay, '#x'))
}
