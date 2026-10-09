// @tier 3
// @covers U20
// defineAsyncComponent renders nothing until its loader resolves on the fake clock, then renders the component.
import Lazy from './Lazy.vue'
import AsyncHost from './AsyncHost.vue'
export async function run() {
  const m = testHost.mount(AsyncHost, { loader: () => new Promise((resolve) => testHost.timers.setTimeout(() => resolve(Lazy), 20)) })
  trace('before', m.html())
  await testHost.timers.advance(20)
  await testHost.tick()
  trace('after', m.html())
}
