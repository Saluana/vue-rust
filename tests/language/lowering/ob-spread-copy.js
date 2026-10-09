const base = { x: 1, y: { deep: true } }
const copy = { ...base, x: 2 }
const more = { ...null, ...undefined, ...'ab', z: 3 }
copy.y.deep = false
trace('spread', copy, base.y.deep)
trace('spread-string', more)
const { x, ...rest } = copy
trace('rest', x, rest)
