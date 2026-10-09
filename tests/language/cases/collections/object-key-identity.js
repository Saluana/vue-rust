// @tier 2
// @covers L11
// Objects are Map and Set keys by identity: an equal-looking object is a different key.
const k1 = { id: 1 }
const k2 = { id: 1 }
const m = new Map([[k1, 'first']])
m.set(k2, 'second')
trace('identity', m.size, m.get(k1), m.get({ id: 1 }), m.has(k2))
const s = new Set([k1, k1, k2])
trace('set-identity', s.size, [...s][0] === k1)
trace('map-contents', m)
