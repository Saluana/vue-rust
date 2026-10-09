// @tier 2
// @covers L10
// Default sort compares strings by UTF-16 code units, puts undefined after values and holes last; sort is stable.
const words = ['b', 'a', 'B', 'aa', 'Z']
trace('default', [...words].sort())
trace('numbers-default', [10, 9, 1, 100].sort())
trace('comparator', [10, 9, 1, 100].sort((x, y) => x - y))
trace('undef-hole', [3, undefined, 1, , 2].sort())
trace('stable', [{k: 1, n: 'a'}, {k: 0, n: 'b'}, {k: 1, n: 'c'}].sort((p, q) => p.k - q.k).map((o) => o.n))
