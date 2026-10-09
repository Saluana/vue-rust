// @tier 2
// @covers V14,V15
// once: true calls back once, and the watcher then stops itself.
import {ref, watch} from 'vue'
const n = ref(0)
const log = []
watch(n, (v) => log.push(v), {once: true})
export async function run() {
  n.value = 1
  await testHost.tick()
  n.value = 2
  await testHost.tick()
  trace('once', log)
}
