// @tier 1
// @covers V03
// Map get and has are tracked per key, and size is tracked for the whole collection. set, delete and clear rerun them.
import {reactive, watchSyncEffect} from 'vue'
const m = reactive(new Map([['a', 1]]))
const log = []
watchSyncEffect(() => log.push(`get-a=${m.get('a')}`))
watchSyncEffect(() => log.push(`size=${m.size}`))
watchSyncEffect(() => log.push(`has-z=${m.has('z')}`))
m.set('a', 2)
m.set('z', 3)
m.delete('a')
m.clear()
trace('log', log, m.size)
