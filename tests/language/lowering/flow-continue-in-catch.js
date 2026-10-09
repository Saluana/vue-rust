let out = ''
for (let i = 0; i < 3; i++) {
  try {
    if (i === 1) throw 'skip'
    out += `ok${i};`
  } catch (e) {
    out += 'caught;'
    continue
  } finally {
    out += 'f;'
  }
  out += 'tail;'
}
trace('continue-catch', out)
