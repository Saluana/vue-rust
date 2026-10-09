function classify(v) {
  let out = ''
  switch (v) {
    case 1:
      out += 'one;'
    case 2:
      out += 'two;'
      break
    default:
      out += 'def;'
    case 3:
      out += 'three;'
  }
  return out
}
trace('s1', classify(1), classify(2), classify(3), classify(9))
let hits = ''
switch ('x') {
  case 'y': hits += 'no'
  case 'x': { const local = 'block'; hits += local }
  case 'z': hits += '+z'
}
trace('s-str', hits)
let calls = ''
const pick = (t) => { calls += `${t};`; return t }
switch (2) {
  case pick(1): break
  case pick(2): break
  case pick(3): break
}
trace('s-lazy', calls)
