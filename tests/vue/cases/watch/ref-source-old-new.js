// @tier 2
// @covers V14
// watch on a ref passes the new and the old value. Writes made before the flush coalesce into one call with the last value.
import {ref, watch} from 'vue'
const n = ref(0)
const log = []
watch(n, (v, old) => log.push([v, old]))
export async function run() {
  n.value = 1
  n.value = 2
  n.value = 3
  await testHost.tick()
  trace('coalesced', log)
  n.value = 4
  await testHost.tick()
  trace('next', log)
}
