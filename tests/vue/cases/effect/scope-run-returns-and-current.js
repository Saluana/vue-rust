// @tier 1
// @covers V12
// scope.run returns the callback's value and makes the scope current. Outside run there is no current scope, and a stopped scope does not run.
import {effectScope, getCurrentScope} from 'vue'
const scope = effectScope()
trace('outside', getCurrentScope() === undefined)
const got = scope.run(() => {
  trace('inside', getCurrentScope() === scope)
  return 42
})
trace('returned', got, getCurrentScope() === undefined)
scope.stop()
trace('stopped-run', scope.run(() => 1) === undefined)
