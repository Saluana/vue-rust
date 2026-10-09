const o = { 0: 'zero', 1: 'one' }
trace('index-keys', o[0], o['1'], o[1.0], o['01'])
const p = {}
p[10] = 'ten'
p[9] = 'nine'
p.x = 'x'
trace('order', p[10], p[9], p.x)
