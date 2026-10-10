// @tier 1
// @covers V12
// effect(runner) builds a new effect from the runner's function, so stopping one runner leaves the other active. An effect
// created inside another effect is not disposed when the outer one reruns: each rerun adds one more inner effect.
import {effect, ref, stop} from 'vue'
const a = ref(1)
const b = ref(0)
let runs = 0
const first = effect(() => {
  runs += 1
  return a.value
})
const second = effect(first)
trace('distinct', first === second, first.effect === second.effect)
trace('after-create', runs)
stop(first)
a.value = 2
trace('after-write-first-stopped', runs)
let innerRuns = 0
let outerRuns = 0
effect(() => {
  outerRuns += 1
  b.value
  effect(() => {
    innerRuns += 1
    b.value
  })
})
trace('nested-created', outerRuns, innerRuns)
b.value = 1
trace('nested-first-write', outerRuns, innerRuns)
