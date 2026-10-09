const o = {}
o['2'] = 'a'
o[2.0] = 'b'
o[2e0] = 'c'
o['2.0'] = 'd'
trace('canonical-keys', o)
