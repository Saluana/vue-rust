// @tier 3
// @covers V14,V17
// A pre watcher runs as a job that is queued at the first trigger. Promise jobs queued before the write run before it, and jobs queued after it run after it.
import {nextTick, ref, watch} from 'vue'
const n = ref(0)
const log = []
watch(n, () => log.push('watch'))
export async function run() {
  new Promise((resolve) => resolve()).then(() => log.push('then-1'))
  n.value = 1
  nextTick(() => log.push('nexttick'))
  new Promise((resolve) => resolve()).then(() => log.push('then-2'))
  await testHost.tick()
  trace('order', log)
}
