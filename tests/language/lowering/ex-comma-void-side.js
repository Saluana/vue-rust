let side = ''
const f = (t) => {
  side += t
  return t
}
const v = (f('a'), f('b'), f('c'))
trace('comma', v, side)
const w = void f('d')
trace('void-side', w, side)
