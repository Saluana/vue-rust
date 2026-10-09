// @tier 1
// @covers V03
// Set clear, values and entries track the whole collection. clear reruns a size reader and both iterating readers.
import {reactive, watchSyncEffect} from 'vue'
const s = reactive(new Set([1, 2]))
const log = []
watchSyncEffect(() => log.push(`size=${s.size}`))
watchSyncEffect(() => {
  const vs = []
  for (const v of s.values()) vs.push(v)
  log.push(`values=${vs.join('|')}`)
})
watchSyncEffect(() => {
  const es = []
  for (const [k] of s.entries()) es.push(k)
  log.push(`entries=${es.join('|')}`)
})
s.clear()
s.add(3)
trace('log', log)
