// @tier 2
// @covers U17
// Removing every keyed item and adding the same keys back creates new elements, so the old references are not reused.
import Keyed from './Keyed.vue'
export async function run() {
  const reg = {}
  const m = testHost.mount(Keyed, { items: ['a', 'b'], reg })
  const old = reg.a
  m.setProps({ items: [], reg })
  await testHost.tick()
  trace('empty', m.html())
  m.setProps({ items: ['a', 'b'], reg })
  await testHost.tick()
  trace('again', m.html(), reg.a !== old)
}
