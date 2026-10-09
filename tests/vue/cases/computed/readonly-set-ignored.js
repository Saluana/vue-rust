// @tier 1
// @covers V06
// A computed without a setter ignores writes silently in production; its value stays the same.
import {computed, ref} from 'vue'
const n = ref(1)
const c = computed(() => n.value + 1)
c.value = 100
trace('ignored', c.value, n.value)
