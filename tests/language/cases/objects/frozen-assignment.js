// @tier 2
// @covers L04
// Object.freeze blocks writes to its own properties; module code is strict, so the failed write throws a TypeError.
const o = Object.freeze({ n: 1, inner: { m: 1 } })
try {
  o.n = 2
} catch (e) {
  trace('write-error', e.name)
}
o.inner.m = 5
trace('state', o, Object.isFrozen(o), Object.isFrozen(o.inner))
