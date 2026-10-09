// @tier 1
// @covers L04
// Spread and Object.assign copy own enumerable properties shallowly; nested objects stay shared.
const base = { x: 1, y: { deep: true } }
const copy = { ...base, x: 2 }
const merged = Object.assign({}, base, { z: 3 })
copy.y.deep = false
trace('spread', copy)
trace('assign', merged, base.y === merged.y)
