// @tier 1
// @covers L09
// Strings are UTF-16 code units: an astral emoji has length 2 and two indexable halves.
const s = 'a😀b'
trace('length', s.length, '😀'.length, s.indexOf('b'))
trace('units', s.charCodeAt(1), s.charCodeAt(2), s.charCodeAt(3))
trace('index', s[1] === '\uD83D', s[2] === '\uDE00', s[3])
