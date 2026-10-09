// @tier 1
// @covers V06
// A computed that returns a new object notifies its readers on every recompute. One that returns the same object does not.
import {computed, ref, watchSyncEffect} from 'vue'
const n = ref(1)
const KEEP = {tag: 'k'}
const fresh = computed(() => ({v: n.value}))
const same = computed(() => {
  n.value
  return KEEP
})
let freshRuns = 0
let sameRuns = 0
watchSyncEffect(() => {
  fresh.value
  freshRuns += 1
})
watchSyncEffect(() => {
  same.value
  sameRuns += 1
})
n.value = 2
trace('runs', freshRuns, sameRuns)
