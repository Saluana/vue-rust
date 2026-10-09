let out = ''
let i = 0
outer: do {
  i += 1
  let j = 0
  while (j < 3) {
    j += 1
    if (j === 2) continue outer
    out += `${i}${j};`
  }
} while (i < 2)
trace('do-while-labels', out)
