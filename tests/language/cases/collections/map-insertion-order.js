// @tier 1
// @covers L11
// Map keeps insertion order; updating a key keeps its position; delete then set moves the key to the end.
const m = new Map()
m.set('b', 1).set('a', 2).set('c', 3)
m.set('a', 20)
trace('order', m)
m.delete('b')
m.set('b', 10)
trace('moved', m, m.size, m.get('zz'), m.has('c'))
trace('keys', [...m.keys()], [...m.values()])
