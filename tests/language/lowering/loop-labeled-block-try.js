let log = ''
found: {
  try {
    log += 'in-try;'
    break found
  } finally {
    log += 'fin;'
  }
  log += 'unreached;'
}
trace('labeled-block-try', log)
