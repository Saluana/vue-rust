// @tier 3
// @covers V14,V17
// Fake timers: the job queue drains after each timer callback, so a pre watcher runs before the next timer fires.
import {ref, watch} from 'vue'
const n = ref(0)
const log = []
watch(n, (v) => log.push(`watch:${v}`))
export async function run() {
  const t = testHost.timers
  t.setTimeout(() => {
    n.value = 1
    log.push('timer-1')
  }, 5)
  t.setTimeout(() => log.push(`timer-2:${n.value}`), 5)
  await t.advance(5)
  trace('log', log)
}
