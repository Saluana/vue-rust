// @tier 1
// @covers L18
// Math results on exact inputs; rounding helpers differ on negative halves and produce negative zero.
trace('round', Math.round(2.5), Math.round(-2.5), Math.round(-0.4), Math.round(0.49999999999999994))
trace('floor-ceil', Math.floor(-1.5), Math.ceil(-1.5), Math.trunc(-1.5), Math.sign(-3))
trace('abs-sqrt', Math.abs(-7), Math.sqrt(16), Math.sqrt(-1), Math.pow(2, 10))
trace('minmax', Math.max(), Math.min(), Math.max(1, '3', 2), Math.min(0, -0))
trace('consts', Math.PI, Math.E > 2.7, Math.hypot(3, 4))
