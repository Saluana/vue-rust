// @tier 2
// @covers U30,U14
// A component whose only root is a v-if false branch renders the comment placeholder.
import Root from './Root.vue'
export async function run() {
  const m = testHost.mount(Root, { show: false })
  trace('hidden', m.html())
  m.setProps({ show: true })
  await testHost.tick()
  trace('shown', m.html())
}
