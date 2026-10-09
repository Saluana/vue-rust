const log = { n: 0 }
function f() {
  try {
    return log
  } finally {
    log.n = 1
  }
}
trace('return-object', f() === log, log.n)
