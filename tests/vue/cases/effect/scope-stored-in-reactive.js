// @tier 1
// @covers V03,V12
// reactive() never wraps an effect scope (EffectScope is __v_skip), so a scope read back from a reactive object is the scope
// itself: run, active and stop work through that read, and the watchers created inside run stop with the scope.
import {effectScope, getCurrentScope, reactive, ref, watchSyncEffect} from 'vue'
const n = ref(1)
const holder = reactive({scope: null})
holder.scope = effectScope()
const scope = holder.scope
trace('same-identity', holder.scope === scope)
let runs = 0
const got = holder.scope.run(() => {
  trace('current-is-scope', getCurrentScope() === scope)
  watchSyncEffect(() => {
    n.value
    runs += 1
  })
  return 42
})
trace('returned', got)
trace('active-before', holder.scope.active)
n.value = 2
trace('runs-before-stop', runs)
holder.scope.stop()
trace('active-after', holder.scope.active)
n.value = 3
trace('runs-after-stop', runs)
trace('run-after-stop-is-undefined', holder.scope.run(() => 1) === undefined)
