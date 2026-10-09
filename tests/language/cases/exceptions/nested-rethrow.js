// @tier 2
// @covers L13
// A rethrow keeps the original value; every handler on the way sees that same object by identity.
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
    trace('outer-finally')
  }
} catch (e2) {
  trace('outer-catch', e2 === original, e2.reason)
}
