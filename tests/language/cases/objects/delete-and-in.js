// @tier 1
// @covers L04
// delete removes an own property; `in` distinguishes present keys (even with undefined values) from missing ones.
const o = { a: 1, b: undefined }
trace('before', 'a' in o, 'b' in o, 'c' in o)
delete o.a
trace('after', 'a' in o, o.a, Object.keys(o))
trace('inherited-in', 'toString' in o)
