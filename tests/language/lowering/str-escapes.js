const s = 'tab\there\nnew "quote" \\ back A \x42'
trace('escapes', s, s.length)
trace('unicode-long', '\u{1F600}', '\u{1F600}'.length)
