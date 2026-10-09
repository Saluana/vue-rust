// @tier 2
// @covers L04
// Destructuring defaults apply only to undefined; the rest object keeps the remaining own enumerable keys.
const src = { a: 1, b: null, c: undefined, d: 4 }
const { a, b = 'dflt', c = 'cdef', ...rest } = src
trace('pieces', a, b, c, rest)
const [first = 'none', , third] = [undefined, 'skip', 'third']
trace('array-pattern', first, third)
