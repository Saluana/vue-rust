// @tier 2
// @covers V16
// A watchEffect created inside a watch callback runs its getter with its own watcher. When that nested run returns,
// getCurrentWatcher is the outer callback's watcher again.
import {getCurrentWatcher, ref, watch, watchEffect} from 'vue'
const n = ref(0)
const m = ref(0)
let outer = null
let innerIsOwn = 'unset'
let restored = 'unset'
watch(n, () => {
  outer = getCurrentWatcher()
  watchEffect(() => {
    innerIsOwn = getCurrentWatcher() !== outer
    m.value
  })
  restored = getCurrentWatcher() === outer
})
export async function run() {
  n.value = 1
  await testHost.tick()
  m.value = 1
  await testHost.tick()
  trace('nested', innerIsOwn, restored, outer !== null, getCurrentWatcher() === undefined)
}
