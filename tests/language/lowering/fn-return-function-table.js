const ops = {
  add: (a, b) => a + b,
  mul: (a, b) => a * b,
}
trace('table', ops.add(2, 3), ops.mul(2, 3), ops.sub === undefined)
