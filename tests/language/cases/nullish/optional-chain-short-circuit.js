// @tier 2
// @covers L06
// Optional chaining stops at a nullish base, so the argument expressions of the rest of the chain are not evaluated.
const log = []
function arg(tag) {
  log.push(tag)
  return tag
}
const none = null
const some = {
  f(x) {
    return `got ${x}`
  },
}
trace('short', none?.f(arg('skipped')), none?.[arg('index')])
trace('present', some?.f(arg('ran')))
trace('log', log)
