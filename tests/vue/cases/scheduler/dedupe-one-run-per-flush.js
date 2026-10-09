// @tier 2
// @covers V14,V17
// Many writes to the sources of one watcher queue it once. The callback runs once per flush and sees the final values.
import {ref, watch} from 'vue'
const a = ref(0)
const b = ref(0)
const log = []
watch([a, b], ([x, y]) => log.push(`${x}-${y}`))
export async function run() {
  a.value = 1
  b.value = 1
  a.value = 2
  await testHost.tick()
  trace('one-flush', log)
}
