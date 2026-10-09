// @tier 1
// @covers V12,V13
// A watcher created inside a scope is created with that scope current. Reruns happen outside the scope, where no scope is current.
import {effectScope, getCurrentScope, ref, watchSyncEffect} from 'vue'
const n = ref(0)
const scope = effectScope()
let seenScope = null
scope.run(() => {
  watchSyncEffect(() => {
    n.value
    seenScope = getCurrentScope()
  })
})
trace('creation', seenScope === scope)
n.value = 1
trace('rerun', seenScope === undefined)
