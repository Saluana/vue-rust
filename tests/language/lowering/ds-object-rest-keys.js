const { a, 0: zero, ...others } = { a: 1, 0: 'z', b: 2, c: 3 }
trace('rest-keys', a, zero, others)
