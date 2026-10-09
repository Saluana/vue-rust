let closes = ''
const src = {
  [Symbol.iterator]() {
    let i = 0
    return {
      next() {
        i += 1
        return { value: i, done: i > 5 }
      },
      return() {
        closes += 'r;'
        throw 'ignored-by-throw-completion'
      },
    }
  },
}
try {
  for (const v of src) {
    if (v === 2) throw `body ${v}`
  }
} catch (e) {
  trace('forof-throw', e, closes)
}
let nested = ''
outer: for (const a of src) {
  for (const b of src) {
    if (b === 2) continue outer
    if (a === 3) break outer
    nested += `${a}${b};`
  }
}
trace('forof-labels', nested, closes)
