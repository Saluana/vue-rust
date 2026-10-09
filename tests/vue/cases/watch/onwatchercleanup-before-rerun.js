// @tier 2
// @covers V16
// onWatcherCleanup inside a watch callback runs before the next call of that callback, and when the watcher stops.
import {onWatcherCleanup, ref, watch} from 'vue'
const n = ref(0)
const log = []
const handle = watch(n, (v) => {
  log.push(`run:${v}`)
  onWatcherCleanup(() => log.push(`cleanup:${v}`))
})
export async function run() {
  n.value = 1
  await testHost.tick()
  n.value = 2
  await testHost.tick()
  handle.stop()
  trace('log', log)
}
