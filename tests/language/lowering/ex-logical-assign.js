let x = 0
x ||= 5
let y = 2
y &&= y + 1
let z = null
z ??= 'set'
const o = { p: 0, q: 'keep' }
o.p ||= 9
o.q &&= 'changed'
let calls = 0
const f = () => { calls += 1; return 1 }
let w = 1
w ||= f()
trace('logical', x, y, z, o.p, o.q, w, calls)
