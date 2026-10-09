const counter = {
  count: 0,
  inc() {
    this.count += 1
    return this
  },
  get double() {
    return this.count * 2
  },
}
counter.inc().inc()
trace('methods', counter.count, counter.double)
const m = counter.inc
let err = ''
try {
  m()
} catch (e) {
  err = e.name
}
trace('detached', err)
