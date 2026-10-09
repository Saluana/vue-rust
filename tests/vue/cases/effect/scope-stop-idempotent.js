// @tier 1
// @covers V12
// Stopping a scope twice runs its cleanups once. A stopped scope does not run its callback, so nothing is registered.
import {effectScope, onScopeDispose} from 'vue'
let disposed = 0
const scope = effectScope()
scope.run(() => {
  onScopeDispose(() => {
    disposed += 1
  })
})
scope.stop()
scope.stop()
trace('disposed', disposed)
scope.run(() => {
  onScopeDispose(() => {
    disposed += 10
  })
})
scope.stop()
trace('still', disposed)
