function readLater() {
  return laterValue
}
try {
  readLater()
} catch (e) {
  trace('call-before-init', e.name)
}
const laterValue = 'now'
trace('after-init', readLater())
