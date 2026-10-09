// @tier 2
// @covers U22
// A disabled Teleport renders its content in place; enabling it moves the content to the target.
import Page from './Page.vue'
export async function run() {
  const overlay = testHost.container('overlay')
  const m = testHost.mount(Page, { off: true })
  trace('disabled', m.html(), overlay.html())
  m.setProps({ off: false })
  await testHost.tick()
  trace('enabled', m.html(), overlay.html())
}
