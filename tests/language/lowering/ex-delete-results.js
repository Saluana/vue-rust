const o = { a: 1 }
const r = [delete o.a, delete o.a, delete o.none]
trace('delete-results', r, 'a' in o)
