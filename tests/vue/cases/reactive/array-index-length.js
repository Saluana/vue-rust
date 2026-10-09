// @tier 1
// @covers V03
// Index and length are tracked. Writing past the end grows length. Setting length truncates and leaves index 0 alone.
import {reactive, watchSyncEffect} from 'vue'
const arr = reactive([1, 2])
const log = []
watchSyncEffect(() => log.push(`len=${arr.length}`))
watchSyncEffect(() => log.push(`first=${arr[0]}`))
arr[4] = 9
arr.length = 1
trace('log', log, arr.length)
