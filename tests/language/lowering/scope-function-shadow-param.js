const name = 'global'
function f(name) {
  const inner = () => name
  name = name + '!'
  return inner()
}
trace('param-shadow', f('arg'), name)
function g(a, b = () => a) {
  a = 'changed'
  return b()
}
trace('default-closure-sees-param', g('orig'))
