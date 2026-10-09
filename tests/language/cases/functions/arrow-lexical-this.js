// @tier 2
// @covers L02
// Arrow functions take this from the enclosing scope; an ordinary call in strict module code has this undefined.
const obj = {
  name: 'outer',
  regular() {
    const arrow = () => this.name
    return [this.name, arrow()]
  },
  arrowProp: () => typeof this,
}
trace('method', obj.regular())
trace('arrow-prop-this', obj.arrowProp())
function plain() {
  return this
}
trace('plain-call-this', plain() === undefined)
