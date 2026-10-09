// @tier 2
// @covers V13,V16
// watchEffect tracks the synchronous reads of each run and reruns on change. Its cleanup runs before each rerun and on stop.
import {onWatcherCleanup, ref, watchEffect} from 'vue'
const n = ref(1)
const log = []
const handle = watchEffect(() => {
  const v = n.value
  log.push(`run:${v}`)
  onWatcherCleanup(() => log.push(`cleanup:${v}`))
})
export async function run() {
  n.value = 2
  await testHost.tick()
  handle.stop()
  trace('log', log)
}
