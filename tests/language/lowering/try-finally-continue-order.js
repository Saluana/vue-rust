let s = ''
for (let i = 0; i < 2; i++) {
  try {
    s += `t${i};`
    continue
  } finally {
    s += `f${i};`
  }
}
trace('continue-order', s)
