function outer(a, f = () => a + 1) {
  const a2 = a * 2
  return [f(), a2]
}
trace('nested-defaults', outer(1), outer(5, () => 'custom'))
