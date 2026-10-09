const arrowName = () => 1
let assigned
assigned = function () {}
const holder = { method: () => 2, ['comp' + 'uted']: function () {} }
trace('names', arrowName.name, assigned.name, holder.method.name, holder.computed.name)
const { destr = () => 3 } = {}
trace('default-name', destr.name)
