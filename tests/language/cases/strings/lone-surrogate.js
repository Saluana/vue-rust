// @tier 2
// @covers L09
// A lone surrogate is a legal string value; trace escapes it as \udXXX exactly like JSON.stringify.
const lone = '\uD800'
trace('lone', lone, lone.length, lone.charCodeAt(0))
trace('pair', '😀' === '😀', '\uDE00'.charCodeAt(0))
trace('mixed', [lone + 'x', 'ok\uDC00'])
