trace('special', 1 / 0, -1 / 0, 0 / 0, 1 / -0, -0 === 0, 1 / (0 * -1))
trace('nan', NaN === NaN, NaN !== NaN, NaN == NaN, typeof NaN)
trace('inf-math', Infinity - Infinity, Infinity * 0, -Infinity < 0)
