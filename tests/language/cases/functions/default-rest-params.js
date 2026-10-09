// @tier 2
// @covers L02
// Default parameters apply on undefined only; rest collects the remaining arguments; length stops at a default.
function join(sep = '-', ...parts) {
  return parts.join(sep)
}
trace('defaults', join(), join('+'), join('+', 'a', 'b'))
function pick(a, b = a * 2, c = undefined) {
  return [a, b, c]
}
trace('pick', pick(1), pick(1, undefined, 3), pick(1, 0, null))
trace('lengths', join.length, pick.length)
