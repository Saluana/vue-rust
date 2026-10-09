let out = ''
for (let i = 0; i < 4; i++) {
  switch (i) {
    case 1:
      continue
    case 2:
      out += 'two;'
      break
    default:
      out += `d${i};`
  }
  out += `end${i};`
}
trace('nested-switch', out)
