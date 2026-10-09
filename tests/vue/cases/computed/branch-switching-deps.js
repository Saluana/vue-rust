// @tier 1
// @covers V06
// Dependencies follow the branch taken: a computed stops depending on the branch it no longer reads.
import {computed, ref} from 'vue'
const useA = ref(true)
const a = ref('a1')
const b = ref('b1')
let runs = 0
const picked = computed(() => {
  runs += 1
  return useA.value ? a.value : b.value
})
trace('start', picked.value, runs)
b.value = 'b2'
trace('unused-branch-write', picked.value, runs)
useA.value = false
trace('switched', picked.value, runs)
a.value = 'a2'
trace('old-branch-write', picked.value, runs)
b.value = 'b3'
trace('new-branch-write', picked.value, runs)
