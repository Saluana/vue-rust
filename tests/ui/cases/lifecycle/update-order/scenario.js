// @tier 2
// @covers V18
// An update of a prop re-renders the parent and the child; beforeUpdate runs before updated in each component.
import Parent from './Parent.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const m = testHost.mount(Parent, { log, n: 1 })
  m.setProps({ log, n: 2 })
  await testHost.tick()
  trace('text', m.html())
}
