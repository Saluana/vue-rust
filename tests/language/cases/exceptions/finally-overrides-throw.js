// @tier 2
// @covers L13
// A return inside finally discards a pending throw; a plain throw passes through finally and reaches the catch.
function swallow() {
  try {
    throw 'lost'
  } finally {
    return 'kept'
  }
}
trace('swallow', swallow())
try {
  try {
    throw { code: 7 }
  } finally {
    trace('cleanup')
  }
} catch (e) {
  trace('caught', e)
}
