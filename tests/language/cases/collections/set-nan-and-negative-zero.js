// @tier 1
// @covers L11
// Set and Map use SameValueZero: NaN equals NaN, and -0 is the same key as 0 (stored as +0).
const s = new Set([NaN, NaN, 0, -0, '0', 0])
trace('set', s, s.size, s.has(NaN), s.has(-0))
const m = new Map([
  [NaN, 'nan'],
  [-0, 'zero'],
])
trace('map-keys', m.get(NaN), m.get(0), [...m.keys()])
