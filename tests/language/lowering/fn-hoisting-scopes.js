trace('hoisted-fn', typeof later, later(2))
function later(x) {
  return x * 10
}
trace('var-before', typeof v, v)
var v = 2
trace('var-after', v)
function outer() {
  trace('inner-var', typeof g, g === undefined)
  if (true) {
    var g = 'inner'
  }
  return g
}
trace('outer', outer())
{
  trace('block-fn', blockFn())
  function blockFn() {
    return 'block'
  }
}
