// @tier 2
// @covers V16
// The third argument of a watch callback registers a cleanup. It runs before the next call and when the watcher stops.
import {ref, watch} from 'vue'
const n = ref(0)
const log = []
const handle = watch(n, (v, old, onCleanup) => {
  log.push(`run:${v}`)
  onCleanup(() => log.push(`cleanup:${v}`))
})
export async function run() {
  n.value = 1
  await testHost.tick()
  n.value = 2
  await testHost.tick()
  handle.stop()
  trace('log', log)
}
