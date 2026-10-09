// @tier 2
// @covers V15
// pause() holds deliveries. Writes made while paused are not delivered, and resume() then delivers the current value.
import {ref, watch} from 'vue'
const n = ref(0)
const log = []
const handle = watch(n, (v) => log.push(v))
export async function run() {
  handle.pause()
  n.value = 1
  await testHost.tick()
  n.value = 2
  n.value = 3
  await testHost.tick()
  trace('paused', log)
  handle.resume()
  await testHost.tick()
  trace('resumed', log)
  n.value = 4
  await testHost.tick()
  trace('after', log)
}
