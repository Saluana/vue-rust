// @tier 2
// @covers L11
// Entries added during Map or Set iteration are visited; entries deleted before being reached are skipped.
const s = new Set([1, 2, 3])
const visitedSet = []
for (const v of s) {
  visitedSet.push(v)
  if (v === 1) {
    s.delete(2)
    s.add(4)
  }
}
trace('set-visit', visitedSet, [...s])
const m = new Map([
  ['a', 1],
  ['b', 2],
])
const visitedMap = []
for (const [k] of m) {
  visitedMap.push(k)
  if (k === 'a') {
    m.set('c', 3)
    m.delete('b')
  }
}
trace('map-visit', visitedMap, [...m.keys()])
