// @tier 1
// @covers L09
// Common String methods on ASCII input with explicit results.
const s = '  Hello, World  '
trace('trim', s.trim(), s.trimStart().length, s.trimEnd().length)
trace('search', s.includes('World'), s.startsWith('  H'), s.endsWith('d  '), s.indexOf('o'), s.lastIndexOf('o'))
trace('split', 'a,b,,c'.split(','), 'abc'.split(''), 'x'.repeat(3), 'ab'.padStart(4, '*'), 'ab'.padEnd(4, '-'))
trace('case', 'MiXeD'.toLowerCase(), 'abc'.slice(-1), 'abc'.charAt(5) === '')
