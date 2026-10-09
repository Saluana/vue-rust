// @tier 1
// @covers L01
// switch falls through until break; labeled continue and break reach outer loops.
function classify(n) {
  const out = []
  switch (n) {
    case 1:
      out.push('one')
    case 2:
      out.push('two-or-fallthrough')
      break
    default:
      out.push('default')
  }
  return out
}
trace('switch-1', classify(1))
trace('switch-2', classify(2))
trace('switch-9', classify(9))
let hits = 0
outer: for (let a = 0; a < 3; a++) {
  for (let b = 0; b < 3; b++) {
    if (b === 1) continue outer
    if (a === 2) break outer
    hits += 1
  }
}
trace('labels', hits)
