// @tier 2
// @covers V14,V17
// A watcher that writes its own source reruns until its guard stops it. The job queue terminates with the expected count.
import {ref, watch} from 'vue'
const n = ref(0)
let runs = 0
watch(n, (v) => {
  runs += 1
  if (v < 5) n.value = v + 1
})
export async function run() {
  n.value = 1
  await testHost.tick()
  trace('recursive', runs, n.value)
}
