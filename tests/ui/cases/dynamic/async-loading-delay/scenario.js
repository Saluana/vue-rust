// @tier 3
// @covers U20
// The loading component appears only after the delay; the real component replaces it when the loader resolves.
import Lazy from './Lazy.vue'
import AsyncDelay from './AsyncDelay.vue'
export async function run() {
  const m = testHost.mount(AsyncDelay, { loader: () => new Promise((resolve) => testHost.timers.setTimeout(() => resolve(Lazy), 100)) })
  trace('t0', m.html())
  await testHost.timers.advance(50)
  await testHost.tick()
  trace('t50', m.html())
  await testHost.timers.advance(60)
  await testHost.tick()
  trace('t110', m.html())
}
