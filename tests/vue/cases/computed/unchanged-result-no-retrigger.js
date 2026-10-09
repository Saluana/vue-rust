// @tier 1
// @covers V06
// When a recomputed value is unchanged, the effects that read the computed are not rerun.
import {computed, ref, watchSyncEffect} from 'vue'
const n = ref(1)
const parity = computed(() => n.value % 2)
let runs = 0
watchSyncEffect(() => {
  parity.value
  runs += 1
})
n.value = 3
trace('same-parity', runs)
n.value = 4
trace('changed-parity', runs, parity.value)
