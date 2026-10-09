// @tier 2
// @covers L18
// JSON.stringify keeps own key order, skips undefined and functions, and writes NaN as null.
const data = { b: [1, 'two', null], a: undefined, f: () => 1, n: NaN, nested: { ok: true } }
const text = JSON.stringify(data)
trace('text', text)
trace('parsed', JSON.parse(text))
trace('indent', JSON.stringify([1, { x: 2 }], null, 2))
trace('primitives', JSON.stringify('é\n'), JSON.stringify(null), JSON.stringify(undefined) === undefined)
