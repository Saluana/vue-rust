function join(sep = '-', ...parts) {
  return `${parts.length}${sep}${parts[0]}${sep}${parts[1]}`
}
trace('defaults', join(), join('+'), join('+', 'a', 'b'))
function pick(a, b = a * 2, c = undefined) {
  return `${a}:${b}:${c}`
}
trace('pick', pick(1), pick(1, undefined, 3), pick(1, 0, null))
function tdz(x = y, y = 1) {
  return x
}
try {
  tdz()
} catch (e) {
  trace('param-tdz', e.name)
}
trace('lengths', pick.length, join.length)
