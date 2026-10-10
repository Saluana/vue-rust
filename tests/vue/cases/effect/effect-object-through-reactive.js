// @tier 1
// @covers V03,V12
// The effect behind a runner is a class instance, which reactive() wraps. Its pause and stop methods run through that proxy and
// act on the effect: a paused effect queues its rerun, and a stopped effect no longer reruns.
import {effect, reactive, ref} from 'vue'
const n = ref(0)
let runs = 0
const runner = effect(() => {
  runs += 1
  return n.value
})
const holder = reactive({effect: runner.effect})
trace('wrapped', holder.effect === runner.effect)
n.value = 1
trace('runs-live', runs)
holder.effect.pause()
n.value = 2
trace('runs-paused', runs)
holder.effect.stop()
n.value = 3
trace('runs-stopped', runs)
