// @tier 2
// @covers U12
// Slot content re-renders when the parent state it reads changes, even though the child did not change.
import Page from './Page.vue'
export async function run() {
  const m = testHost.mount(Page)
  testHost.click(testHost.find(m, '#inc'))
  testHost.click(testHost.find(m, '#inc'))
  await testHost.tick()
  trace('html', m.html())
}
