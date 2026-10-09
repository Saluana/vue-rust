// @tier 1
// @covers L13
// Any value can be thrown and catch receives it unchanged, including null, undefined and numbers.
function thrower(v) {
  throw v
}
const values = ['text', 42, null, undefined, { code: 'E1' }]
const caught = []
for (const v of values) {
  try {
    thrower(v)
  } catch (e) {
    caught.push(e)
  }
}
trace('caught', caught)
