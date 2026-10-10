// @tier 2
// @covers A10,A11,V01
// A render function builds a keyed list from a ref. Reversing the array moves the existing li elements: the node identities stay the same and only their order changes.
import List from './List.vue'
export async function run() {
  const m = testHost.mount(List)
  const before = testHost.findAll(m, 'li')
  trace('before', m.html())
  testHost.click(testHost.find(m, '#rev'))
  await testHost.tick()
  const after = testHost.findAll(m, 'li')
  trace('after', m.html())
  trace('texts', after.map((n) => n.text()))
  trace('same-nodes', before[0].exists(), after.length)
}
