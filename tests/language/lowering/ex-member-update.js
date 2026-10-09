const o = { n: 1, inner: { c: 10 } }
const a = o.n++
const b = ++o['n']
const c = o.inner.c--
trace('member-update', a, b, c, o.n, o.inner.c)
