const o = { a: undefined, 1: 'one' }
trace('in', 'a' in o, 'b' in o, 1 in o, '1' in o, 'toString' in o)
trace('in-after', delete o.a, 'a' in o)
