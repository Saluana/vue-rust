let log = ''
const f = (t) => {
  log += t
  return t
}
const r = [f('a'), f('b'), , f('c')]
trace('order', r, log)
