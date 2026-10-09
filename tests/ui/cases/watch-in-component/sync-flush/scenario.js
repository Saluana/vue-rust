// @tier 1
// @covers V13,V14
// A sync watcher runs during the write itself, before any flush, so it sees the new value immediately.
import { ref } from 'vue'
import Sync from './Sync.vue'
export function run() {
  const src = ref(0)
  testHost.mount(Sync, { src, log: (v) => trace('log', v) })
  src.value = 1
  trace('after-write')
  src.value = 2
}
