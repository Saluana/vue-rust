// @tier 2
// @covers V13
// watchEffect collects its dependencies again on every run. A branch it no longer reads stops triggering it.
import {ref, watchEffect} from 'vue'
const useA = ref(true)
const a = ref('a')
const b = ref('b')
const log = []
watchEffect(() => log.push(useA.value ? a.value : b.value))
export async function run() {
  b.value = 'b2'
  await testHost.tick()
  useA.value = false
  await testHost.tick()
  a.value = 'a2'
  await testHost.tick()
  b.value = 'b3'
  await testHost.tick()
  trace('log', log)
}
