// @tier 3
// @covers V14,V17
// A write made inside a promise job starts a flush that runs after the promise jobs already queued.
import {ref, watch} from 'vue'
const n = ref(0)
const log = []
watch(n, (v) => log.push(`watch:${v}`))
export async function run() {
  new Promise((resolve) => resolve()).then(() => {
    n.value = 1
    log.push('job-wrote')
  })
  new Promise((resolve) => resolve()).then(() => log.push('job-2'))
  await testHost.tick()
  trace('order', log)
}
