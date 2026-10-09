// @tier 1
// @covers V03
// Map forEach and entries track the whole collection: an add, a delete, or a changed value reruns the iterating effect.
import {reactive, watchSyncEffect} from 'vue'
const m = reactive(new Map())
const keys = []
watchSyncEffect(() => {
  const ks = []
  m.forEach((v, k) => ks.push(k))
  keys.push(ks.join(','))
})
const pairs = []
watchSyncEffect(() => {
  const ps = []
  for (const [k, v] of m.entries()) ps.push(`${k}=${v}`)
  pairs.push(ps.join(','))
})
m.set('x', 1)
m.set('y', 2)
m.delete('x')
m.set('y', 3)
trace('keys', keys)
trace('pairs', pairs)
