let log = ''
try {
  undefinedFunction()
} catch (e) {
  log += `${e.name};`
}
try {
  const c = 1
  c = 2
} catch (e) {
  log += `${e.name};`
}
try {
  ({}).x.y
} catch (e) {
  log += `${e.name};`
}
try {
  throw new RangeError('custom')
} catch (e) {
  log += `${e.name}:${e.message};`
}
trace('kinds', log)
