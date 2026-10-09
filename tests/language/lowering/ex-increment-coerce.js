let v = '5'
const old = v++
trace('postfix-string', old, v, typeof v)
let o = { n: null }
const p = o.n++
trace('null-increment', p, o.n)
let bigish = 2 ** 53
bigish++
trace('precision', bigish)
