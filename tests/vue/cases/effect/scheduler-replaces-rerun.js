// @tier 1
// @covers V12
// With a scheduler option, a write calls the scheduler instead of rerunning fn. The runner runs fn when called.
import {effect, ref} from 'vue'
const n = ref(0)
let runs = 0
let scheduled = 0
const runner = effect(() => {
  runs += 1
  return n.value
}, {
  scheduler: () => {
    scheduled += 1
  },
})
trace('after-create', runs, scheduled)
n.value = 1
n.value = 2
trace('after-two-writes', runs, scheduled)
trace('runner-value', runner())
trace('after-runner-call', runs, scheduled)
