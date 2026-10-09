function level1(a) {
  return function level2(b) {
    return (c) => {
      return () => a + b + c
    }
  }
}
trace('depth', level1('a')('b')('c')())
let counter = 0
const bump = () => (counter += 1)
const twice = () => {
  bump()
  return bump()
}
trace('bump', twice(), counter)
