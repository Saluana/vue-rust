let n = 10
let steps = 0
while (n > 0 && steps < 100) {
  n = n % 2 === 0 ? n / 2 : n - 1
  steps += 1
}
trace('collatz', n, steps)
let x = 0
do x += 2
while (x < 7)
trace('do-no-braces', x)
for (;;) {
  x += 1
  if (x > 9) break
}
trace('infinite-break', x)
