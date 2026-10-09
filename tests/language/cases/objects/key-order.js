// @tier 1
// @covers L04
// Own key order: integer-like keys ascending first, then string keys in insertion order.
const o = { b: 1, 10: 'ten', a: 2, 2: 'two' }
o.c = 3
o[1] = 'one'
trace('keys', Object.keys(o))
trace('entries', Object.entries(o))
trace('object', o)
