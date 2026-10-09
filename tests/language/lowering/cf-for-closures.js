const fns = [0, 0, 0]
for (let i = 0; i < 3; i++) {
  fns[i] = () => i
}
trace('let-capture', fns[0](), fns[1](), fns[2]())
const vf = [0, 0]
for (var j = 0; j < 2; j++) {
  vf[j] = () => j
}
trace('var-capture', vf[0](), vf[1](), j)
let total = 0
for (let k = 0, m = 10; k < m; k += 3, m -= 1) {
  total += k * m
}
trace('two-vars', total)
