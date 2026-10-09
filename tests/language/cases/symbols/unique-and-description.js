// @tier 1
// @covers L12
// Symbols are unique even with equal descriptions; trace records only an ordinal and the description.
const a = Symbol('tag')
const b = Symbol('tag')
const none = Symbol()
trace('unique', a === b, a === a, typeof a)
trace('symbols', a, b, none)
trace('desc', a.description, none.description)
const o = { [a]: 'hidden', visible: 1 }
trace('keys', Object.keys(o), o[a], o[b] === undefined)
