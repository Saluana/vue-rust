// @tier 2
// @covers U05
// After a re-render the handler reads the current props. setProps replaces the whole props object, so the listener is passed again.
import Latest from './Latest.vue'
export async function run() {
  const m = testHost.mount(Latest, { label: 'one', onHit: (v) => trace('hit', v) })
  testHost.click(testHost.find(m, '#b'))
  m.setProps({ label: 'two', onHit: (v) => trace('hit', v) })
  await testHost.tick()
  testHost.click(testHost.find(m, '#b'))
}
