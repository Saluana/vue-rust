// @tier 2
// @covers L13
// finally runs after a return; a return inside finally replaces the pending completion.
function f() {
  try {
    return 'try'
  } finally {
    trace('finally-f')
  }
}
function g() {
  try {
    return 'try'
  } finally {
    return 'finally'
  }
}
trace('f', f())
trace('g', g())
