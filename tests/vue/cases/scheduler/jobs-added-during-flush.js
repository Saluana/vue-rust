// @tier 2
// @covers V14,V17
// Jobs queued during a flush run in the same flush, after the jobs already queued, in queue order.
import {ref, watch} from 'vue'
const a = ref(0)
const b = ref(0)
const c = ref(0)
const log = []
watch(a, (v) => {
  log.push(`a:${v}`)
  b.value = v
  c.value = v
})
watch(b, (v) => log.push(`b:${v}`))
watch(c, (v) => log.push(`c:${v}`))
export async function run() {
  a.value = 1
  await testHost.tick()
  trace('order', log)
}
