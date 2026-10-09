let a, b, c
a = b = c = 5
const o = {}
o.x = o.y = 'shared'
trace('chain-assign', a, b, c, o.x, o.y)
let k = (a = 9) + 1
trace('assign-value', k, a)
