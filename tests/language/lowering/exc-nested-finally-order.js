let log = ''
function inner() {
  try {
    log += 'i-try;'
    throw 'x'
  } finally {
    log += 'i-fin;'
  }
}
function outer() {
  try {
    inner()
  } catch (e) {
    log += `o-catch:${e};`
    return 'handled'
  } finally {
    log += 'o-fin;'
  }
  return 'unreached'
}
trace('order', outer(), log)
