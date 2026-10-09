const o = { a: { b: 5 }, f() { return 'called' } }
trace('chain', o?.a?.b, o.x?.y, o.x?.y.z.w, o.f?.(), o.g?.())
let log = ''
const n = null
trace('short', n?.[(log += 'skip;', 0)], log)
