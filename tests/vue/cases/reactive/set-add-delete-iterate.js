// @tier 1
// @covers V03
// Set add, delete, has, size and forEach are tracked. Adding a duplicate is not a change.
import {reactive, watchSyncEffect} from 'vue'
const s = reactive(new Set(['a']))
const log = []
watchSyncEffect(() => log.push(`size=${s.size}`))
watchSyncEffect(() => log.push(`has-b=${s.has('b')}`))
watchSyncEffect(() => {
  const xs = []
  s.forEach((v) => xs.push(v))
  log.push(`items=${xs.join('|')}`)
})
s.add('a')
s.add('b')
s.delete('a')
trace('log', log, s.size)
