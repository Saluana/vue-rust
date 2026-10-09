let out = ''
outer: for (let i = 0; i < 3; i++) {
  for (let j = 0; j < 3; j++) {
    try {
      if (j === 1) continue outer
      if (i === 2) break outer
      out += `${i}${j};`
    } finally {
      out += '.'
    }
  }
}
trace('labels-try', out)
