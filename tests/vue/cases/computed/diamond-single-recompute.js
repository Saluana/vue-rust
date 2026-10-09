// @tier 1
// @covers V06
// A diamond (two paths from one source into one computed) recomputes the join once per change of the source.
import {computed, ref} from 'vue'
const src = ref(1)
let joinRuns = 0
const left = computed(() => src.value + 1)
const right = computed(() => src.value * 2)
const join = computed(() => {
  joinRuns += 1
  return left.value + right.value
})
trace('init', join.value, joinRuns)
src.value = 5
trace('after', join.value, joinRuns)
