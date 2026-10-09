// @tier 1
// @covers V12
// Vue 3.5.43 ignores the lazy option: effect(fn, {lazy: true}) still runs fn before it returns, and a write reruns it.
import {effect, ref} from 'vue'
const n = ref(0)
let runs = 0
const runner = effect(() => {
  runs += 1
  return n.value
}, {lazy: true})
trace('after-lazy-create', runs, typeof runner)
n.value = 1
trace('after-write', runs)
