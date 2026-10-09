// @tier 2
// @covers V14
// A deep watcher fires for a nested write to the same reactive object it watches.
import { reactive } from 'vue'
import Deep from './Deep.vue'
export async function run() {
  const state = reactive({ k: 1, nested: { x: 1 } })
  const m = testHost.mount(Deep, { state, log: (v) => trace('log', v) })
  state.nested.x = 2
  await testHost.tick()
  state.k = 5
  await testHost.tick()
  trace('text', m.html())
}
