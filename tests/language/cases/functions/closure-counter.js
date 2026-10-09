// @tier 1
// @covers L02
// Closures keep their own mutable state; two counters created by one factory do not share it.
function makeCounter(start) {
  let n = start
  return {
    next() {
      n += 1
      return n
    },
    peek: () => n,
  }
}
const a = makeCounter(0)
const b = makeCounter(10)
trace('a1', a.next())
trace('a2', a.next())
trace('b1', b.next())
trace('peek', a.peek(), b.peek())
