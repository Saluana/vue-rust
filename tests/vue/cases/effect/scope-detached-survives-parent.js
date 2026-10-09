// @tier 1
// @covers V12
// A detached scope is not stopped with its parent. It has to be stopped on its own.
import {effectScope, onScopeDispose} from 'vue'
const log = []
const parent = effectScope()
let independent = null
parent.run(() => {
  independent = effectScope(true)
  independent.run(() => {
    onScopeDispose(() => log.push('independent-disposed'))
  })
})
parent.stop()
trace('after-parent-stop', log)
independent.stop()
trace('after-own-stop', log)
