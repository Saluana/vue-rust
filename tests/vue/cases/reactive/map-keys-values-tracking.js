// @tier 1
// @covers V03
// Map keys() and values() track the collection as entries() and forEach() do. A changed value reruns a values() reader.
import {reactive, watchSyncEffect} from 'vue'
const m = reactive(new Map([['a', 1]]))
const keys = []
watchSyncEffect(() => keys.push([...m.keys()].join(',')))
const values = []
watchSyncEffect(() => values.push([...m.values()].join(',')))
m.set('b', 2)
m.set('a', 5)
m.delete('b')
trace('keys', keys)
trace('values', values)
