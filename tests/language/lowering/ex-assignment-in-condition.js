let i = 0
while ((i += 1) < 3) {}
trace('assign-cond', i)
let t
if ((t = 'set')) trace('assign-if', t)
