let i = 1
const r = [i++, ++i, i--, --i, i]
trace('update', r, i)
let s = '3'
s++
trace('string-update', typeof s, s)
let u
u++
trace('undef-update', u)
