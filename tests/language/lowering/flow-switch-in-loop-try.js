let out = ''
for (let i = 0; i < 3; i++) {
  try {
    switch (i) {
      case 0:
        continue
      case 1:
        out += 'one;'
        break
      default:
        out += 'def;'
    }
    out += 'after;'
  } finally {
    out += `f${i};`
  }
}
trace('switch-try', out)
