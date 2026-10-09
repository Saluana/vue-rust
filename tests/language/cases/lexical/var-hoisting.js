// @tier 1
// @covers L01
// var is hoisted and initialized to undefined; function-scoped vars escape blocks.
trace('var-before', typeof v, v)
var v = 2
trace('var-after', v)
function f() {
  trace('f-before', typeof g, g === undefined)
  if (true) {
    var g = 'inner'
  }
  trace('f-after', g)
  return g
}
trace('f-result', f())
trace('hoisted-fn', typeof hoisted, hoisted())
function hoisted() {
  return 'ok'
}
