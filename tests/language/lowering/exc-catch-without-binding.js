let count = 0
try {
  throw 'a'
} catch {
  count += 1
}
try {
  throw 'b'
} catch {
  count += 10
} finally {
  count += 100
}
trace('count', count)
