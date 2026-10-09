const key = 'a'
const o = { a: 1 }
trace('in-expr', key in o, 'b' in o || 'miss')
