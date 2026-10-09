const o = { n: 1, s: 'a' }
o.n += 4
o.s += 'b'
o['n'] *= 2
const k = 'n'
o[k] -= 3
trace('member', o.n, o.s)
