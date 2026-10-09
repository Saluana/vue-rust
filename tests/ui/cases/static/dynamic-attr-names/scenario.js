// @tier 2
// @covers U03
// A dynamic attribute name: changing the name removes the old attribute; the same-name shorthand :id binds the id variable.
import Dyn from './Dyn.vue'
export async function run() {
  const m = testHost.mount(Dyn, { name: 'data-x', val: 'a' })
  trace('first', m.html())
  m.setProps({ name: 'title', val: 'b' })
  await testHost.tick()
  trace('second', m.html())
}
