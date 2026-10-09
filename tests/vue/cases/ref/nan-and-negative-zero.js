// @tier 1
// @covers V01
// Writes compare with Object.is: NaN equals NaN (no change), and -0 differs from 0 (a change).
import {ref, watchSyncEffect} from 'vue'
const n = ref(0)
let runs = 0
watchSyncEffect(() => {
  n.value
  runs += 1
})
n.value = 0
n.value = -0
n.value = -0
const nan = ref(NaN)
let nanRuns = 0
watchSyncEffect(() => {
  nan.value
  nanRuns += 1
})
nan.value = NaN
trace('runs', runs, nanRuns, n.value, nan.value)
