// @tier 1
// @covers L13
// Engine-raised errors are traced by name and class only; their message text is V8-specific and is never recorded.
let caught = null
try {
  null.property
} catch (e) {
  caught = e
  trace('engine', e.name, e instanceof TypeError)
}
trace('after', caught instanceof Error, caught !== null)
