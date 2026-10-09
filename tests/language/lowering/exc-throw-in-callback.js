function each(n, cb) {
  for (let i = 0; i < n; i++) cb(i)
}
let seen = ''
try {
  each(5, (i) => {
    seen += i
    if (i === 2) throw 'stop'
  })
} catch (e) {
  trace('callback-throw', e, seen)
}
