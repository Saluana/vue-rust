let log = ''
const k = { toString() { log += 'tostr;'; return 'n' } }
const o = { n: 1 }
function rhs() { log += 'rhs;'; return 2 }
o[k] += rhs()
trace('order-compound', log, o.n)
