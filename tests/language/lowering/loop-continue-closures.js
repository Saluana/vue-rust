const fs = [0, 0, 0, 0]
let k = 0
for (let i = 0; i < 4; i++) {
  if (i % 2 === 0) continue
  fs[k] = () => i
  k += 1
}
trace('continue-capture', fs[0](), fs[1]())
