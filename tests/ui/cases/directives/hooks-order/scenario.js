// @tier 2
// @covers U26
// Custom directive hooks run in Vue's order: created, beforeMount, mounted, then beforeUpdate and updated on change, and the unmount pair last.
import Trace from './Trace.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const m = testHost.mount(Trace, { n: 1, log })
  m.setProps({ n: 2, log })
  await testHost.tick()
  m.unmount()
  trace('done')
}
