function make(limit) {
  let count = 0
  return function next() {
    count += 1
    if (count > limit) throw `over ${count}`
    return count
  }
}
const n = make(2)
let msg = ''
for (let i = 0; i < 4; i++) {
  try {
    n()
  } catch (e) {
    msg = e
  }
}
trace('limit', msg)
