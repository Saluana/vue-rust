// @tier 3
// @covers V14,V17
// nextTick(fn) runs fn after the flush that was pending when it was called, so fn sees the watcher's output.
import {nextTick, ref, watch} from 'vue'
const n = ref(0)
const log = []
watch(n, (v) => log.push(`watch:${v}`))
export async function run() {
  n.value = 1
  nextTick(() => log.push(`tick:${n.value}`))
  n.value = 2
  await testHost.tick()
  trace('log', log)
}
