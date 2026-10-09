// @tier 3
// @covers U20
// A loader that rejects shows the error component.
import Lazy from './ErrComp.vue'
import AsyncErr from './AsyncErr.vue'
export async function run() {
  const m = testHost.mount(AsyncErr, { loader: () => new Promise((resolve, reject) => testHost.timers.setTimeout(() => reject(new Error('network')), 5)) })
  trace('before', m.html())
  await testHost.timers.advance(5)
  await testHost.tick()
  trace('after', m.html())
}
