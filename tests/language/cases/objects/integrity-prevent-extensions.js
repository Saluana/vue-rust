// @tier 2
// @covers L04
// preventExtensions blocks new keys but leaves existing ones writable; an empty non-extensible object counts as frozen.
const open = { a: 1 }
Object.preventExtensions(open)
open.a = 2
try {
  open.b = 3
} catch (e) {
  trace('add-blocked', e.name)
}
const empty = Object.preventExtensions({})
const items = Object.preventExtensions([7, 8])
items[0] = 9
trace('prevent', open, empty, items, Object.isExtensible(empty))
const outer = { inner: Object.freeze({ m: 1 }), free: { n: 2 } }
trace('nested', outer)
