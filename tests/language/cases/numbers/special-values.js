// @tier 1
// @covers L08
// NaN never equals itself; division by zero yields infinities rather than errors; -0 is observable through 1/x.
trace('nan', NaN === NaN, NaN !== NaN, typeof NaN, 0 / 0)
trace('inf', 1 / 0, -1 / 0, Infinity - Infinity, Infinity > 1e308)
trace('zero', 0 === -0, 5 / -0, -5 % 5, Math.sign(-0))
trace('arith', 2 ** -1, 7 / 2, Math.trunc(-7 / 2), Math.floor(-7 / 2))
