// @tier 1
// @covers V06
// An error thrown by a getter reaches the reader with the same value. After the source changes the computed recovers.
import {computed, ref} from 'vue'
const n = ref(0)
let boom = null
const c = computed(() => {
  if (n.value < 0) {
    boom = new Error('negative')
    throw boom
  }
  return n.value + 1
})
trace('ok', c.value)
n.value = -1
try {
  trace('no-throw', c.value)
} catch (e) {
  trace('caught', e === boom, e.name)
}
n.value = 4
trace('recovered', c.value)
