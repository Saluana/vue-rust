// @tier 1
// @covers V12
// Stopping an effect scope stops the watch effects created inside it: a later write does not rerun them.
import {effectScope, ref, watchSyncEffect} from 'vue'
const n = ref(1)
let runs = 0
const scope = effectScope()
scope.run(() => {
  watchSyncEffect(() => {
    n.value
    runs += 1
  })
})
n.value = 2
trace('before-stop', runs)
scope.stop()
n.value = 3
trace('after-stop', runs, n.value)
