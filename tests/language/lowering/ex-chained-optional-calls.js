const api = { user: { name: 'ann', greet(p) { return `hi ${p}` } } }
trace('chain-calls', api.user?.greet?.('bo'), api.admin?.greet('x'), api.user.missing?.())
