// @tier 1
// @covers L01
// Reading a let binding before its declaration throws an engine ReferenceError: trace only name and instanceof.
trace('before', 1)
try {
  trace('read', value)
} catch (e) {
  trace('caught', e.name, e instanceof ReferenceError)
}
let value = 2
trace('after', value)
