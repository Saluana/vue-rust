// @tier 2
// @covers V18,V19,U14
// Mount and unmount hooks follow a v-if toggle of the child, one pair per appearance.
import Toggle from './Toggle.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const m = testHost.mount(Toggle, { show: false, log })
  m.setProps({ show: true, log })
  await testHost.tick()
  m.setProps({ show: false, log })
  await testHost.tick()
  trace('html', m.html())
}
