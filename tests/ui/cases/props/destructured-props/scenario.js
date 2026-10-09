// @tier 2
// @covers U11,S07
// Destructured props stay reactive: the template reads the current value, and a destructuring default applies when the prop is absent.
import Dest from './Dest.vue'
export async function run() {
  const m = testHost.mount(Dest, { label: 'a' })
  trace('first', testHost.find(m, '#p').text())
  m.setProps({ size: 'lg', label: 'b' })
  await testHost.tick()
  trace('second', testHost.find(m, '#p').text())
}
