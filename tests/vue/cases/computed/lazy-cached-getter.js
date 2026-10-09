// @tier 1
// @covers V06
// A computed runs its getter lazily, once per change: nothing runs until the first read, and reads in between are cached.
import {computed, ref} from 'vue'
let calls = 0
const n = ref(1)
const doubled = computed(() => {
  calls += 1
  return n.value * 2
})
trace('before-read', calls)
trace('reads', doubled.value, doubled.value, doubled.value, calls)
n.value = 2
trace('stale-read-not-yet', calls)
trace('after-change', doubled.value, calls)
