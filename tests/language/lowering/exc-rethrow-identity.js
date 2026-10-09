const original = { reason: 'disk' }
function inner() {
  try {
    throw original
  } catch (e) {
    trace('inner-sees', e === original)
    throw e
  }
}
try {
  try {
    inner()
  } finally {
    trace('outer-finally', true)
  }
} catch (e2) {
  trace('outer-catch', e2 === original, e2.reason)
}
