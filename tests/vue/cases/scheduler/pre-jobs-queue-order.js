// @tier 2
// @covers V14,V17
// Pre jobs with no component id run in the order they were queued, which is the order of the writes that triggered them.
import {ref, watch} from 'vue'
const x = ref(0)
const y = ref(0)
const log = []
watch(x, () => log.push('x'))
watch(y, () => log.push('y'))
export async function run() {
  y.value = 1
  x.value = 1
  await testHost.tick()
  trace('first-order', log)
  log.length = 0
  x.value = 2
  y.value = 2
  await testHost.tick()
  trace('second-order', log)
}
