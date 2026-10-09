const obj = {
  name: 'outer',
  regular() {
    const arrow = () => this.name
    return `${this.name}/${arrow()}`
  },
  arrowProp: () => typeof this,
}
trace('method', obj.regular())
trace('arrow-prop', obj.arrowProp())
function plain() {
  return this
}
trace('plain-this', plain() === undefined)
const bound = { v: 'x', get() { return () => this.v } }
trace('nested-arrow', bound.get()())
