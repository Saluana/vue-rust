// @tier 2
// @covers L12
// Symbol.for returns the same symbol for the same key; Symbol() never does.
const r1 = Symbol.for('app.key')
const r2 = Symbol.for('app.key')
trace('registry', r1 === r2, r1 === Symbol('app.key'), r1.description)
trace('ids', r1, r2)
const table = new Map([[r1, 'registered']])
trace('lookup', table.get(r2), table.size)
