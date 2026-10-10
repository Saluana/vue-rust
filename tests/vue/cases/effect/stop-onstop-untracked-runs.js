// @tier 1
// @covers V12
// stop(runner) drops the dependencies and runs onStop once; stopping again does nothing. A later call of the runner runs fn without tracking it.
import {effect, ref, stop} from 'vue'
const n = ref(0)
let runs = 0
let stops = 0
const runner = effect(() => {
  runs += 1
  return n.value
}, {
  onStop: () => {
    stops += 1
  },
})
stop(runner)
stop(runner)
trace('after-stop', runs, stops)
n.value = 5
trace('after-write-while-stopped', runs, stops)
trace('untracked-runner-value', runner(), runs)
n.value = 6
trace('after-write-after-untracked-run', runs, stops)
