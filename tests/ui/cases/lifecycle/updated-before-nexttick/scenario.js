// @tier 3
// @covers V17,V18
// The updated hook runs in the flush, so a nextTick callback registered after the write runs after updated.
import Upd from './Upd.vue'
export async function run() {
  const m = testHost.mount(Upd, { log: (v) => trace('log', v) })
  testHost.click(testHost.find(m, '#b'))
  await testHost.tick()
  trace('html', m.html())
}
