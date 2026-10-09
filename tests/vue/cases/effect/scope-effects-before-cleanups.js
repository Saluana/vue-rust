// @tier 1
// @covers V12
// On stop, the scope's effects are stopped before its cleanups run, so a write made by a cleanup reruns nothing.
import {effectScope, onScopeDispose, ref, watchSyncEffect} from 'vue'
const log = []
const n = ref(0)
const scope = effectScope()
scope.run(() => {
  watchSyncEffect(() => {
    n.value
    log.push('effect-run')
  })
  onScopeDispose(() => {
    log.push('cleanup')
    n.value = 99
  })
})
log.push('before-stop')
scope.stop()
log.push(`after-stop-${n.value}`)
trace('order', log)
