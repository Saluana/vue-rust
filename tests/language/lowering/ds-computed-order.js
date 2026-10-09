let log = ''
const key = (k) => {
  log += `k${k};`
  return k
}
const src = { a: 1, b: 2 }
const { [key('a')]: first, [key('b')]: second = (log += 'dflt;', 0) } = src
trace('computed', first, second, log)
let rlog = ''
const { ...spread } = { p: 1, q: 2 }
trace('rest-copy', spread.q, rlog === '')
