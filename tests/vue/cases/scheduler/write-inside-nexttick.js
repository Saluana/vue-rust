// @tier 3
// @covers V14,V17
// A write inside a nextTick callback starts a new flush. A nextTick awaited before that flush runs sees the write but not the watcher.
import {nextTick, ref, watch} from 'vue'
const n = ref(0)
const log = []
watch(n, (v) => log.push(`watch:${v}`))
export async function run() {
  nextTick(() => {
    n.value = 5
    log.push('wrote')
  })
  await nextTick()
  trace('after-first', log)
  await testHost.tick()
  trace('after-second', log)
}
