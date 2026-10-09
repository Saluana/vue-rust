const pick = (c) => (c ? 'yes' : 'no')
trace('cond', pick(1), pick(0), pick(''), pick('x'))
let t = (1, 2, 3)
trace('seq', t)
const deep = true ? false ? 'a' : 'b' : 'c'
trace('nested-cond', deep)
