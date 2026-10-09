// @tier 1
// @covers L03
// Two references to one object see each other's writes; assignment copies the reference, not the object.
const a = { n: 1 }
const b = a
b.n = 2
trace('shared', a.n, a === b)
const list = [a]
list[0].n = 3
trace('through-array', a.n, list[0] === b)
