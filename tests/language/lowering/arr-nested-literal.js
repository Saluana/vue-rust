const m = [[1, [2]], { k: [3] }]
m[0][1][0] = 'changed'
trace('nested', m, m[1].k[0])
