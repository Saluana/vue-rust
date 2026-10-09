// @tier 2
// @covers L10
// Setting length truncates, or extends with holes; extending never creates undefined elements.
const a = [1, 2, 3, 4]
a.length = 2
trace('truncated', a)
a.length = 4
trace('extended', a, 2 in a)
a[7] = 'far'
trace('sparse', a.length, a)
trace('indexes', Object.keys(a))
