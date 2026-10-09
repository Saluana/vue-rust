const name = 'dyn'
const o = {
  [name + 'Method']() {
    return 'm'
  },
  [name]: 'value',
  [1 + 1]: 'two',
}
trace('computed-methods', o.dynMethod(), o.dyn, o[2])
