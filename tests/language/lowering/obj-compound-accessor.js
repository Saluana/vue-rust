let log = ''
const o = {
  _n: 1,
  get n() {
    log += 'g;'
    return this._n
  },
  set n(v) {
    log += 's;'
    this._n = v
  },
}
o.n += 5
o.n++
trace('compound-accessor', o._n, log)
