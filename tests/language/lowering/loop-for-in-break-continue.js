let out = ''
for (const k in { a: 1, b: 2, c: 3 }) {
  if (k === 'b') continue
  out += k
  if (k === 'c') break
}
trace('forin-jumps', out)
