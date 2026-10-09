// @tier 3
// @covers V16
// onWatcherCleanup called after an await in a watch callback is not tied to that run, so the cleanup is never registered.
import {onWatcherCleanup, ref, watch} from 'vue'
const n = ref(0)
const log = []
const handle = watch(n, async (v) => {
  await null
  onWatcherCleanup(() => log.push(`late-cleanup:${v}`))
  log.push(`run:${v}`)
})
export async function run() {
  n.value = 1
  await testHost.tick()
  n.value = 2
  await testHost.tick()
  handle.stop()
  await testHost.tick()
  trace('log', log)
}
