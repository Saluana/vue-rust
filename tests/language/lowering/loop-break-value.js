function search(target) {
  let found = -1
  for (let i = 0; i < 10; i++) {
    if (i * i === target) {
      found = i
      break
    }
  }
  return found
}
trace('search', search(49), search(50))
