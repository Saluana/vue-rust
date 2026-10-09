// @tier 1
// @covers L02

const a = 1 + 2
let b = a * 3
function f(x) {
  return x + b
}
trace('sum', a, b, f(10))
