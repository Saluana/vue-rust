// @tier 2
// @covers U03
// v-bind with an object spreads its keys as attributes; class merges with the static class and a later class binding.
import Spread from './Spread.vue'
export async function run() {
  const m = testHost.mount(Spread, { opts: { href: '/x', title: 't', class: 'extra', on: true } })
  trace('spread', m.html())
  m.setProps({ opts: { title: 'u', on: false } })
  await testHost.tick()
  trace('respread', m.html())
}
