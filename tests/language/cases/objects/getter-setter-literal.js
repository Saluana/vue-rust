// @tier 2
// @covers L04
// Accessors in object literals run on read and write; trace records an accessor marker and never calls the getter.
const log = []
const temp = {
  c: 20,
  get f() {
    log.push('get f')
    return (this.c * 9) / 5 + 32
  },
  set f(v) {
    log.push(`set f ${v}`)
    this.c = ((v - 32) * 5) / 9
  },
}
trace('before', temp)
temp.f = 212
trace('after-set', temp.f, temp.c)
trace('log', log)
