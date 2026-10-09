// @tier 2
// @covers V14
// A getter source calls back only when the value it returns changes (Object.is), not on every write it depends on.
import {ref, watch} from 'vue'
const n = ref(1)
const log = []
watch(() => n.value > 5, (v, old) => log.push(`${old}->${v}`))
export async function run() {
  n.value = 2
  n.value = 3
  await testHost.tick()
  trace('no-change', log)
  n.value = 6
  await testHost.tick()
  trace('changed', log)
}
