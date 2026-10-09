const src = { a: 1, b: null, c: undefined, d: 4 }
const { a, b = 'dflt', c = 'cdef', ...rest } = src
trace('pieces', a, b, c, rest)
const { p: { q = 'deep' } = {}, ['k' + 1]: renamed = 'none' } = { k1: 'x' }
trace('nested', q, renamed)
let t1, t2
({ t1, t2: t2 = 'set' } = { t1: 'first' })
trace('assign-pattern', t1, t2)
const { length: len } = 'four'
trace('from-string', len)
