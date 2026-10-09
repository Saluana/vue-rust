// @tier 1
// @covers V12
// Stopping a parent scope stops its child scopes. The parent's own cleanups run before the children's.
import {effectScope, onScopeDispose} from 'vue'
const log = []
const parent = effectScope()
parent.run(() => {
  onScopeDispose(() => log.push('parent-1'))
  const child = effectScope()
  child.run(() => {
    onScopeDispose(() => log.push('child-1'))
    onScopeDispose(() => log.push('child-2'))
  })
  onScopeDispose(() => log.push('parent-2'))
})
parent.stop()
trace('order', log)
