// @tier 2
// @covers U11
// setProps replaces the whole props object: a prop left out of the new object becomes absent.
import Pair from './Pair.vue'
export async function run() {
  const m = testHost.mount(Pair, { a: 'x', b: 'y' })
  m.setProps({ a: 'z' })
  await testHost.tick()
  trace('text', testHost.find(m, '#p').text())
}
