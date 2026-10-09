// @tier 3
// @covers V03
// splice, sort and reverse mutate through the proxy. A deep watcher on the array runs once per flush, so the
// intermediate states of sort (whose write order is engine-defined) are never observed.
import {reactive, watch} from 'vue'
const list = reactive([3, 1, 2])
const joined = []
watch(list, () => joined.push(list.join('|')))
export async function run() {
  const removed = list.splice(1, 1, 'x', 'y')
  await testHost.tick()
  list.sort()
  list.reverse()
  await testHost.tick()
  trace('state', removed, [...list], joined)
}
