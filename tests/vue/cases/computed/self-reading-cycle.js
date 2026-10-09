// @tier 1
// @covers V06
// A computed that reads its own value: the inner read returns the value from before this evaluation, not an error or a loop.
import {computed, ref} from 'vue'
const n = ref(1)
const self = computed(() => {
  n.value
  const before = self.value
  return before === undefined ? 100 : before + 1
})
trace('first', self.value)
trace('second', self.value)
n.value = 2
trace('after-change', self.value)
