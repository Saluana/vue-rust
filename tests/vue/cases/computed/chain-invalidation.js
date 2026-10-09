// @tier 1
// @covers V06
// A computed built on a computed invalidates through the chain. A link nobody reads is recomputed only when read.
import {computed, ref} from 'vue'
const base = ref(1)
let aRuns = 0
let bRuns = 0
const a = computed(() => {
  aRuns += 1
  return base.value + 1
})
const b = computed(() => {
  bRuns += 1
  return a.value * 10
})
trace('first', b.value, aRuns, bRuns)
base.value = 2
trace('invalidated', aRuns, bRuns)
trace('second', b.value, aRuns, bRuns)
trace('reread', b.value, aRuns, bRuns)
