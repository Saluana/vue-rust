function thrower(v) {
  throw v
}
let acc = ''
const values = ['t', 1, null, undefined]
for (let i = 0; i < 4; i++) {
  try {
    thrower(values[i])
  } catch (e) {
    acc += `${e};`
  }
}
trace('thrown', acc)
