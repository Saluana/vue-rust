let log = ''
function f1() {
  try {
    return 'try'
  } finally {
    log += 'f1;'
  }
}
function f2() {
  try {
    return 'try'
  } finally {
    return 'finally'
  }
}
function f3() {
  try {
    throw 'lost'
  } finally {
    return 'kept'
  }
}
trace('f', f1(), f2(), f3(), log)
