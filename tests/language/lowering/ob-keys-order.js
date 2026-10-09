const o = { b: 1, 10: 'ten', a: 2, 2: 'two' }
o.c = 3
o[1] = 'one'
trace('object', o)
const n = { 1: 'a', '01': 'b', 1.5: 'c', 0x10: 'd', [`k${1}`]: 'e', 2e0: 'f' }
trace('canonical', n)
const dup = { x: 1, y: 2, x: 3 }
trace('dup', dup)
