// @tier 2
// @covers U23
// The appear hooks run for a Transition that is present on its first render when appear is set.
import Ap from './Ap.vue'
export async function run() {
  const m = testHost.mount(Ap, { log: (v) => trace('log', v) })
  await testHost.tick()
  trace('html', m.html())
}
