// @tier 1
// @covers L02
// Recursive calls see their own binding and accumulate through default parameters.
function fib(n) {
  return n < 2 ? n : fib(n - 1) + fib(n - 2)
}
trace('fib', fib(10), fib(15))
function countDown(n, acc = []) {
  if (n === 0) return acc
  acc.push(n)
  return countDown(n - 1, acc)
}
trace('countdown', countDown(4))
