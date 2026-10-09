let out = ''
for (let i = 0, j = 10; i < j; i += 3, j -= 3) {
  out += `${i}-${j};`
}
trace('seq-for', out)
