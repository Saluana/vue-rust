// @tier 2
// @covers V14,V17
// For one write: sync watchers run during the write, pre watchers run in the job queue, and post watchers run after the queue.
import {ref, watch} from 'vue'
const n = ref(0)
const log = []
watch(n, () => log.push('pre'))
watch(n, () => log.push('post'), {flush: 'post'})
watch(n, () => log.push('sync'), {flush: 'sync'})
export async function run() {
  n.value = 1
  await testHost.tick()
  trace('order', log)
}
