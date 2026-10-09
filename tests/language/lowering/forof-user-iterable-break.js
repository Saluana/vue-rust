let closes = ''
const src = {
  [Symbol.iterator]() {
    let i = 0
    return {
      next() {
        i += 1
        return i <= 3 ? { value: i, done: false } : { value: undefined, done: true }
      },
      return() {
        closes += 'closed;'
        return {}
      },
    }
  },
}
let out = ''
for (const v of src) {
  out += v
  if (v === 2) break
}
trace('forof-break', out, closes)
let all = ''
for (const v of src) {
  if (v === 1) continue
  all += v
}
trace('forof-continue', all, closes)
function firstOver(limit) {
  for (const v of src) {
    if (v > limit) return v
  }
  return -1
}
trace('forof-return', firstOver(1), closes)
