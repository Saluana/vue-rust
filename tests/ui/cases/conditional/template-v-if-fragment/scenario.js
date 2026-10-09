// @tier 2
// @covers U14,U30
// A <template v-if> groups several siblings with no wrapper; a false condition leaves one comment placeholder.
import Frag from './Frag.vue'
export async function run() {
  const m = testHost.mount(Frag, { show: false })
  trace('hidden', m.snapshot())
  m.setProps({ show: true })
  await testHost.tick()
  trace('shown', m.html())
}
