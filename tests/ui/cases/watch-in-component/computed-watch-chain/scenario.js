// @tier 2
// @covers V06,V14
// A watcher on a computed value runs only when the computed result changes.
import Chain from './Chain.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const m = testHost.mount(Chain, { n: 1, log })
  m.setProps({ n: 1, log })
  await testHost.tick()
  m.setProps({ n: 3, log })
  await testHost.tick()
  trace('text', testHost.find(m, '#d').text())
}
