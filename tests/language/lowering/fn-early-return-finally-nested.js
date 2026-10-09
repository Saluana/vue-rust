function f(x) {
  if (x) return 'early'
  for (let i = 0; i < 2; i++) {
    try {
      if (i === 1) return 'late'
    } finally {
      trace('f-fin', i)
    }
  }
  return 'end'
}
trace('early', f(true), f(false))
