// @tier 1
// @covers V12
// effect runs its function once before it returns, reruns it synchronously when a dependency changes, and the runner calls it again and returns its value.
import {effect, ref} from 'vue'
const n = ref(1)
let runs = 0
const runner = effect(() => {
  runs += 1
  return n.value * 10
})
trace('after-create', runs, typeof runner)
n.value = 2
trace('after-write', runs)
trace('runner-value', runner())
trace('after-runner-call', runs)
