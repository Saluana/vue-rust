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
trace('a', a.next(), a.next(), a.peek())
trace('b', b.next(), a.peek(), b.peek())
function shared() {
  let s = 0
  const inc = () => {
    s += 1
  }
  const get = () => s
  return [inc, get]
}
const pair = shared()
pair[0]()
pair[0]()
trace('shared', pair[1]())
