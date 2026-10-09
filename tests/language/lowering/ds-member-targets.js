const holder = { v: 0, w: 0 }
const log = ''
let trail = ''
function key(k) {
  trail += k
  return k
}
;({ a: holder[key('v')], b: holder[key('w')] } = { a: 'A', b: 'B' })
trace('member-targets', holder.v, holder.w, trail)
