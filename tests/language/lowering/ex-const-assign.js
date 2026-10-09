const k = 1
try {
  k = 2
} catch (e) {
  trace('const-error', e.name)
}
trace('const-kept', k)
