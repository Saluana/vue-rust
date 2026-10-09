// @tier 1
// @covers V12
// A nested scope.run restores the outer scope as current when it returns, even when the callback throws.
import {effectScope, getCurrentScope} from 'vue'
const outer = effectScope()
const inner = effectScope()
outer.run(() => {
  inner.run(() => {
    trace('inner-current', getCurrentScope() === inner)
  })
  trace('outer-restored', getCurrentScope() === outer)
  let boom = null
  try {
    inner.run(() => {
      boom = new Error('inside run')
      throw boom
    })
  } catch (e) {
    trace('caught-same', e === boom)
  }
  trace('after-throw', getCurrentScope() === outer)
})
trace('outside', getCurrentScope() === undefined)
