const named = function inner() {
  return typeof inner
}
const arrow = () => 1
const obj = { m() {}, get g() { return 1 }, f: function () {} }
trace('names', named.name, arrow.name, obj.m.name, obj.f.name, named())
trace('lengths', named.length, arrow.length, ((a, b = 1, c) => a).length, ((...r) => r).length)
trace('getter-name', Object.getOwnPropertyDescriptor(obj, 'g').get.name)
