// @tier 2
// @covers U15
// v-show keeps the element in the tree and hides it with an inline display style.
import Show from './Show.vue'
export async function run() {
  const m = testHost.mount(Show, { show: false })
  trace('hidden', m.html())
  m.setProps({ show: true })
  await testHost.tick()
  trace('shown', m.html())
  trace('same-count', testHost.findAll(m, 'p').length, testHost.findAll(m, 'input').length)
}
