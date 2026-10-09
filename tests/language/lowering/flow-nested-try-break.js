let log = ''
for (let i = 0; i < 2; i++) {
  try {
    try {
      log += `a${i};`
      break
    } finally {
      log += 'inner-f;'
    }
  } finally {
    log += 'outer-f;'
  }
}
trace('nested-break', log)
