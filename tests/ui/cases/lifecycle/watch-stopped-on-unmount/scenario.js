// @tier 2
// @covers V15,V18
// A watcher created in a component stops with the component: a later change to its source calls nothing.
import { ref } from 'vue'
import Watch from './Watch.vue'
export async function run() {
  const src = ref(1)
  const m = testHost.mount(Watch, { src, log: (v) => trace('log', v) })
  src.value = 2
  await testHost.tick()
  m.unmount()
  src.value = 3
  await testHost.tick()
  trace('done')
}
