let effects = 0
function g() {
  let v = 'start'
  try {
    return v
  } finally {
    v = 'changed'
    effects += 1
  }
}
trace('return-value', g(), effects)
