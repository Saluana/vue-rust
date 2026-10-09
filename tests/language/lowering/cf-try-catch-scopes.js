let log = ''
try {
  log += 'a;'
  throw 'boom'
} catch (e) {
  log += `c:${e};`
} finally {
  log += 'fin;'
}
trace('catch', log)
try {
  try {
    throw { code: 7 }
  } finally {
    log += 'inner-fin;'
  }
} catch (e) {
  trace('rethrown', e.code, log)
}
try {
  throw 42
} catch (err) {
  const err2 = err + 1
  trace('catch-scope', err, err2)
}
try {
  null.x
} catch (e) {
  trace('engine', e.name, e instanceof TypeError)
}
