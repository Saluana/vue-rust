const s = 'a😀b'
trace('index', s.length, s[0], s[1] === '\uD83D', s[2] === '\uDE00', s[3], s[9])
trace('string-value', s + '!', s === 'a😀b', 'x' < 'y')
