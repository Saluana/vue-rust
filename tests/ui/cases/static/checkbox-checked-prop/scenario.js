// @tier 2
// @covers U03
// Vue 3.5 mirrors a bound checked into the checked attribute as well as the property, so after the update the snapshot has no checked prop.
import Chk from './Chk.vue'
export async function run() {
  const m = testHost.mount(Chk, { on: false })
  trace('off', m.snapshot())
  m.setProps({ on: true })
  await testHost.tick()
  trace('on', m.snapshot(), m.html())
}
