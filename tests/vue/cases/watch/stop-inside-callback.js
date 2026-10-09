// @tier 2
// @covers V14,V15
// A watcher can stop itself from its callback. Later writes no longer reach the callback.
import {ref, watch} from 'vue'
const n = ref(0)
const log = []
const handle = watch(n, (v) => {
  log.push(v)
  if (v >= 2) handle.stop()
})
export async function run() {
  n.value = 1
  await testHost.tick()
  n.value = 2
  await testHost.tick()
  n.value = 3
  await testHost.tick()
  trace('log', log)
}
