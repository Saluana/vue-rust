// @tier 2
// @covers L09
// slice and substring work on code units, so cutting an emoji leaves half a surrogate pair.
const s = 'x😀y'
trace('slice', s.slice(1, 2).length, s.slice(1, 3) === '😀', s.slice(2).charCodeAt(0))
trace('substring', s.substring(3), s.substring(2, 1), s.substring(-2, 2))
trace('upper', 'ß'.length, 'abc'.toUpperCase())
