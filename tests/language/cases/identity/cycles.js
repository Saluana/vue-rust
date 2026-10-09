// @tier 2
// @covers L03
// Cycles are traced by reference: the second visit of an object inside one trace call is a ref to its id.
const node = { name: 'root', child: null }
const kid = { name: 'kid', parent: node }
node.child = kid
trace('cycle', node)
