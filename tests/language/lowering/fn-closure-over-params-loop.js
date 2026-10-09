const make = (n) => {
  const out = [0, 0, 0]
  for (let i = 0; i < 3; i++) {
    out[i] = () => n + i
  }
  return out
}
const fs = make(100)
trace('params-loop', fs[0](), fs[2]())
