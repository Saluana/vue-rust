// @tier 2
// @covers L04
// freeze and seal are traced with integrity markers; a sealed object still accepts writes to its existing keys.
const frozen = Object.freeze({ a: 1, inner: { b: 2 } })
const sealed = Object.seal({ c: 3 })
sealed.c = 4
try {
  sealed.extra = 5
} catch (e) {
  trace('sealed-add', e.name)
}
trace('states', frozen, sealed, Object.isFrozen(frozen), Object.isSealed(sealed), Object.isFrozen(sealed))
const list = Object.freeze([1, 2])
trace('frozen-array', list, Object.isExtensible(list))
