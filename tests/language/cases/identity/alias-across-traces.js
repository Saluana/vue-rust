// @tier 2
// @covers L03
// Object ids are global ordinals; a later trace call expands the same object again with its current state.
const o = { n: 1 }
trace('first', o)
o.n = 2
o.extra = true
trace('second', o, [o])
