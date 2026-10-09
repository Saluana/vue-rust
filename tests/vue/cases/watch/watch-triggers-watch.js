// @tier 2
// @covers V14,V17
// A watcher that writes the source of another watcher: the second watcher runs in the same job queue flush.
import {ref, watch} from 'vue'
const a = ref(0)
const b = ref(0)
const log = []
watch(a, (v) => {
  log.push(`a:${v}`)
  b.value = v * 10
})
watch(b, (v) => log.push(`b:${v}`))
export async function run() {
  a.value = 1
  await testHost.tick()
  trace('chain', log, b.value)
}
