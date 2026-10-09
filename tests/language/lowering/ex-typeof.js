function fn() {}
const o = {}
trace('typeof', typeof 1, typeof 'a', typeof true, typeof undefined, typeof null, typeof o, typeof fn)
trace('undeclared', typeof neverDeclared, typeof neverDeclared === 'undefined')
let tdz = typeof tdzLater
let tdzLater = 1
trace('tdz-typeof-ok', tdz)
