let out = ''
outer: for (let a = 0; a < 3; a++) {
  for (let b = 0; b < 3; b++) {
    if (b === 1) continue outer
    if (a === 2) break outer
    out += `${a}${b};`
  }
}
trace('labels', out)
let n = 0
do {
  n += 1
  if (n % 2 === 0) continue
  out += `d${n};`
} while (n < 5)
trace('do', n, out)
let w = 3
while (w > 0) {
  w -= 1
  if (w === 1) break
}
trace('while', w)
block: {
  trace('in-block', 1)
  break block
}
