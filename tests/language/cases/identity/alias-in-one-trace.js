// @tier 1
// @covers L03
// The same object passed twice in one trace call is expanded once; later occurrences are refs with the same id.
const shared = { k: 'v' }
trace('pair', shared, shared, [shared])
