const o = {}
o[1] = 'num'
o['1'] = 'str'
o[1.5] = 'frac'
o[-0] = 'zero'
o[true] = 'bool'
o[null] = 'nul'
trace('keys', o)
