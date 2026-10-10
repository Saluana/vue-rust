// @tier 2
// @covers V16
// getCurrentWatcher is undefined in the source function of watch(source, cb), which is not wrapped. It is defined in the
// callback and in a watchEffect getter, and undefined again once the callback has returned.
import {getCurrentWatcher, ref, watch, watchEffect} from 'vue'
const n = ref(0)
const seen = []
watch(() => {
  seen.push(getCurrentWatcher() === undefined ? 'source-none' : 'source-some')
  return n.value
}, () => {
  seen.push(getCurrentWatcher() === undefined ? 'callback-none' : 'callback-some')
})
watchEffect(() => {
  seen.push(getCurrentWatcher() === undefined ? 'effect-none' : 'effect-some')
  n.value
})
export async function run() {
  seen.push(getCurrentWatcher() === undefined ? 'outside-before' : 'outside-some')
  n.value = 1
  await testHost.tick()
  seen.push(getCurrentWatcher() === undefined ? 'outside-after' : 'outside-some')
  trace('seen', seen)
}
