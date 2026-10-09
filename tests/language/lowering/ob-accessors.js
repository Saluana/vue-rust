let backing = 20
const temp = {
  get f() {
    return (backing * 9) / 5 + 32
  },
  set f(v) {
    backing = ((v - 32) * 5) / 9
  },
  get only() {
    return 'ro'
  },
}
trace('get', temp.f, temp.only)
temp.f = 212
trace('set', backing, temp.f)
try {
  temp.only = 'ignored'
} catch (e) {
  trace('getter-only-error', e.name)
}
const key = 'dyn'
const d = {
  get [key]() {
    return 'dynamic'
  },
}
trace('computed-getter', d.dyn)
