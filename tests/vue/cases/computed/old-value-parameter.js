// @tier 1
// @covers V06
// The getter receives the previous value as its argument (3.4 and later). The first call receives undefined.
import {computed, ref} from 'vue'
const n = ref(1)
const seen = []
const c = computed((prev) => {
  seen.push(prev)
  return n.value * 10
})
trace('first', c.value)
n.value = 2
trace('second', c.value, seen)
