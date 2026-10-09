function f(list) {
  let i = 0
  try {
    while (true) {
      i += 1
      if (i === 3) return `stop at ${i}`
    }
  } finally {
    trace('loop-finally', i)
  }
}
trace('return-from-loop', f(0))
