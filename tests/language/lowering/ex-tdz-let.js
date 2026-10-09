let beforeRead = 'start'
try {
  trace('before', typeof beforeRead, beforeRead)
  early = early + 1
} catch (e) {
  trace('tdz-error', e.name, e instanceof ReferenceError)
}
let early = 1
trace('after', early)
