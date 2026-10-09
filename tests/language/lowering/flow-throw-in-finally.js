function h() {
  try {
    throw 'first'
  } finally {
    throw 'second'
  }
}
try {
  h()
} catch (e) {
  trace('replaced-throw', e)
}
