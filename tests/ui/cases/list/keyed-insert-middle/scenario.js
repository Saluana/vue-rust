// @tier 2
// @covers U17
// Inserting in the middle of a keyed list creates one element and keeps the elements of the other keys.
import Keyed from './Keyed.vue'
export async function run() {
  const reg = {}
  const m = testHost.mount(Keyed, { items: ['a', 'c'], reg })
  const a = reg.a
  const c = reg.c
  m.setProps({ items: ['a', 'b', 'c'], reg })
  await testHost.tick()
  trace('html', m.html())
  trace('kept', reg.a === a, reg.c === c, reg.b !== undefined)
}
