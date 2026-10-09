// @tier 2
// @covers L18,L21
// Number and String statics and parsing helpers, with explicit results including non-numeric outcomes.
trace('number', Number.isInteger(5.0), Number.isInteger('5'), Number.isSafeInteger(2 ** 53), Number.isNaN('x'), isNaN('x'))
trace('parse', Number('  12  '), Number(''), Number('0x1f'), parseInt('08'), parseInt('1e3'), parseFloat('1e3x'))
trace('stringify', String(null), String([1, [2, 3]]), String({}), [1, null, undefined].join(','))
trace('char-codes', String.fromCharCode(72, 105), 'Hi'.charCodeAt(0), 'abc'.charCodeAt(1))
