const captured = []
for (let i = 0; i < 2; i++) {
  try {
    throw `e${i}`
  } catch (err) {
    captured[i] = () => err
  }
}
trace('catch-capture', captured[0](), captured[1]())
