const fs = [0, 0, 0]
for (let i = 0; i < 3; i++) {
  const local = i * 10
  fs[i] = () => local + i
}
trace('body-capture', fs[0](), fs[1](), fs[2]())
let k = 0
const hs = [0, 0]
while (k < 2) {
  let copy = k
  hs[k] = () => copy
  k += 1
}
trace('while-capture', hs[0](), hs[1]())
