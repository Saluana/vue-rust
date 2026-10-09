// @tier 3
// @covers V14,V17
// await nextTick() resolves only after the pending pre-flush watchers have run.
import {nextTick, ref, watch} from 'vue'
const n = ref(0)
const log = []
watch(n, (v) => log.push(`watch:${v}`))
export async function run() {
  n.value = 1
  log.push('before-nexttick')
  await nextTick()
  log.push('after-nexttick')
  trace('log', log)
}
