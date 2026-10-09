// @tier 2
// @covers U14
// v-if, v-else-if and v-else: a false branch leaves a comment placeholder, so the sibling positions stay fixed.
import Chain from './Chain.vue'
export async function run() {
  const m = testHost.mount(Chain, { mode: 'a' })
  trace('a', m.snapshot())
  m.setProps({ mode: 'b' })
  await testHost.tick()
  trace('b', m.html())
  m.setProps({ mode: 'z' })
  await testHost.tick()
  trace('z', m.html())
}
