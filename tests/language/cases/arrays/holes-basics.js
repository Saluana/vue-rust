// @tier 1
// @covers L10
// A hole is not undefined: forEach, map and filter skip holes, map keeps them, join prints them as empty text.
const a = [1, , 3]
const seen = []
a.forEach((v, i) => seen.push(i))
trace('holes', a, seen, a.length)
trace('map', a.map((v) => v * 2), a.join('-'), 1 in a, a[1] === undefined)
trace('filter', a.filter(() => true).length)
