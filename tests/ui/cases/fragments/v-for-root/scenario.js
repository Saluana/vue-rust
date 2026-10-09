// @tier 2
// @covers U30,U16
// A component whose root is a v-for renders a fragment; the snapshot shows what anchors the fragment has when it is empty.
import List from './List.vue'
export async function run() {
  const m = testHost.mount(List, { n: 2 })
  trace('two', m.html())
  m.setProps({ n: 3 })
  await testHost.tick()
  trace('three', m.html())
  m.setProps({ n: 0 })
  await testHost.tick()
  trace('none', m.html(), m.snapshot())
}
