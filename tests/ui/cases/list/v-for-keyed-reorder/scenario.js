// @tier 2
// @covers U16,U17
// Keyed v-for: reordering and removing items keeps the element of every surviving key (checked through a registry of elements).
import Keyed from './Keyed.vue'
export async function run() {
  const registry = {}
  const items = [{ id: 1, label: 'a' }, { id: 2, label: 'b' }, { id: 3, label: 'c' }]
  const m = testHost.mount(Keyed, { items, registry })
  const first = { 1: registry[1], 2: registry[2], 3: registry[3] }
  trace('mounted', m.html())
  m.setProps({ items: [items[2], items[0], items[1]], registry })
  await testHost.tick()
  trace('reordered', m.html())
  trace('same-after-reorder', registry[1] === first[1], registry[2] === first[2], registry[3] === first[3])
  m.setProps({ items: [items[2]], registry })
  await testHost.tick()
  trace('removed', m.html(), registry[3] === first[3])
}
