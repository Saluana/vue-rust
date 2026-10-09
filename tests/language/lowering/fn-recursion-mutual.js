function isEven(n) {
  return n === 0 ? true : isOdd(n - 1)
}
function isOdd(n) {
  return n === 0 ? false : isEven(n - 1)
}
trace('mutual', isEven(10), isOdd(7))
function fact(n) {
  return n <= 1 ? 1 : n * fact(n - 1)
}
trace('fact', fact(10), fact(20))
const fib = function f(n) {
  return n < 2 ? n : f(n - 1) + f(n - 2)
}
trace('fib', fib(15))
