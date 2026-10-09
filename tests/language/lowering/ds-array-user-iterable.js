let closes = ''
const src = {
  [Symbol.iterator]() {
    let i = 0
    return {
      next() {
        i += 1
        return { value: i * 10, done: i > 3 }
      },
      return() {
        closes += 'x;'
        return {}
      },
    }
  },
}
const [a, b] = src
trace('pattern-two', a, b, closes)
const [first, , third = 'd', ...more] = src
trace('pattern-holes', first, third, more, closes)
let x, y
;[x, y] = src
trace('assign-pattern', x, y)
