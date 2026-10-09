// @tier 2
// @covers L10
// forEach fixes its length at the start; for-of sees elements appended during iteration; splice shifts later indexes.
const a = [1, 2, 3]
const visitedForEach = []
a.forEach((v, i) => {
  visitedForEach.push(v)
  if (i === 0) a.push(99)
})
trace('forEach', visitedForEach, a)
const b = [1, 2, 3]
const visitedOf = []
for (const v of b) {
  visitedOf.push(v)
  if (v === 1) b.push(4)
  if (visitedOf.length > 10) break
}
trace('for-of', visitedOf, b)
const c = ['a', 'b', 'c', 'd']
const removed = []
for (let i = 0; i < c.length; i++) {
  if (c[i] === 'b') removed.push(...c.splice(i, 1))
}
trace('splice-loop', c, removed)
