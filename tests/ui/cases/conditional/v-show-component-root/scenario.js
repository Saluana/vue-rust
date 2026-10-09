// @tier 2
// @covers U15
// v-show on a component applies to the component's root element, and the component stays mounted while hidden.
import Host from './Host.vue'
export async function run() {
  const m = testHost.mount(Host, { show: false })
  trace('hidden', m.html())
  m.setProps({ show: true })
  await testHost.tick()
  trace('shown', m.html())
}
