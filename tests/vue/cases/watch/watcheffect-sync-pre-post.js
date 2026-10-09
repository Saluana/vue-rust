// @tier 3
// @covers V13,V17
// The first run of watchEffect and watchPostEffect is synchronous, at creation. After a write, watchSyncEffect runs during
// the write, watchEffect (pre) runs in the job queue, and watchPostEffect (post) runs after the queue.
import {ref, watchEffect, watchPostEffect, watchSyncEffect} from 'vue'
const n = ref(0)
const log = []
watchEffect(() => log.push(`pre:${n.value}`))
watchPostEffect(() => log.push(`post:${n.value}`))
watchSyncEffect(() => log.push(`sync:${n.value}`))
export async function run() {
  log.push('module-done')
  n.value = 1
  log.push('after-write')
  await testHost.tick()
  trace('log', log)
}
