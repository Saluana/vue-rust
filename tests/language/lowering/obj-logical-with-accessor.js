let sets = ''
const o = {
  _v: 0,
  get v() {
    return this._v
  },
  set v(n) {
    sets += `set${n};`
    this._v = n
  },
}
o.v ||= 4
o.v &&= 5
o.v ??= 6
trace('logical-accessor', o.v, sets)
