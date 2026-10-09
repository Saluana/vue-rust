// @tier 1
// @covers V03
// Property reads, writes and the `in` operator are tracked. Writing an equal value is not a change; adding a key is.
import {reactive, watchSyncEffect} from 'vue'
const o = reactive({a: 1})
const log = []
watchSyncEffect(() => log.push(`a=${o.a}`))
watchSyncEffect(() => log.push(`has-b=${'b' in o}`))
o.a = 2
o.a = 2
o.b = 'new'
trace('log', log)
