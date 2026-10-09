var dup = 1
var dup = dup + 1
trace('redeclared', dup)
function h(p) {
  var p
  return p
}
trace('param-var', h('kept'))
function hv() {
  var z = 'first'
  if (true) {
    var z = 'second'
  }
  return z
}
trace('var-block', hv())
