const result = (function (x) {
  return x + 1
})(41)
trace('iife', result)
function apply(f, v) {
  return f(v)
}
trace('callback', apply((x) => x * 2, 21), apply(function (y) {
  return y - 1
}, 5))
const chain = (a) => (b) => (c) => a + b + c
trace('curry', chain(1)(2)(3))
