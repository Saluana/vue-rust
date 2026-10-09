// @tier 1
// @covers L01
// do-while runs its body first; continue jumps to the condition; break leaves the loop early.
let n = 0
const log = []
do {
  n += 1
  if (n % 2 === 0) continue
  log.push(n)
} while (n < 5)
trace('do-while', n, log)
let w = 3
while (w > 0) {
  w -= 1
  if (w === 1) break
}
trace('while-break', w)
