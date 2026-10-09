// @tier 1
// @covers V06
// A writable computed sends its writes to the setter, and the getter then reads the sources the setter updated.
import {computed, ref} from 'vue'
const first = ref('Ada')
const last = ref('Lovelace')
const full = computed({
  get: () => `${first.value} ${last.value}`,
  set(parts) {
    first.value = parts[0]
    last.value = parts[1]
  },
})
trace('get', full.value)
full.value = ['Grace', 'Hopper']
trace('after-set', full.value, first.value, last.value)
