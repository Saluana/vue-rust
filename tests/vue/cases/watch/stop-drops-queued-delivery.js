// @tier 2
// @covers V15
// A write queues the watcher. Stopping the watcher before the flush drops that delivery, and later writes reach nothing.
import {ref, watch} from 'vue'
const n = ref(0)
const log = []
const handle = watch(n, (v) => log.push(v))
export async function run() {
  n.value = 1
  handle.stop()
  await testHost.tick()
  trace('dropped', log)
  n.value = 2
  await testHost.tick()
  trace('none-after-stop', log)
}
