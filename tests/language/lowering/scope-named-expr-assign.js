const fact = function self(n) {
  if (n <= 1) return 1
  return n * self(n - 1)
}
trace('named-expr', fact(5))
const nameOnly = function nm() {
  try {
    nm = 1
  } catch (e) {
    return e.name
  }
  return 'assigned'
}
trace('self-const', nameOnly())
