// @tier 1
// @covers L13
// A scenario-created Error thrown at module level: name and message are scenario text, so both may be traced.
trace('loading', 'module')
throw new Error('module boom')
