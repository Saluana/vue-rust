// @tier 2
// @covers V14,V17
// The default flush is 'pre': the callback waits for the job queue, so the statement after the write still sees an empty log.
import {ref, watch} from 'vue'
const n = ref(0)
const log = []
watch(n, (v) => log.push(`pre:${v}`))
export async function run() {
  n.value = 1
  trace('sync-after-write', log)
  await testHost.tick()
  trace('after-tick', log)
}
