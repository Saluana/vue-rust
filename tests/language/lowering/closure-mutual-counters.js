let shared = 0
const incA = () => {
  shared += 1
  return shared
}
const incB = () => incA() * 10
trace('mutual-counters', incB(), incA(), shared)
