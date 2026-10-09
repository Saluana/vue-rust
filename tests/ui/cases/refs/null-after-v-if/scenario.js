// @tier 2
// @covers U18,U14
// A template ref is null again once its element is removed by v-if.
import Toggle from './Toggle.vue'
export async function run() {
  const m = testHost.mount(Toggle, { show: true })
  trace('shown', testHost.findAll(m, 'p').length)
  m.setProps({ show: false })
  await testHost.tick()
  trace('hidden', testHost.findAll(m, 'p').length)
}
