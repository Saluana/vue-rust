// @tier 1
// @covers V03
// Object.keys, for-in and object spread track key membership: adding or deleting a key reruns each reader.
import {reactive, watchSyncEffect} from 'vue'
const o = reactive({a: 1})
const seen = []
watchSyncEffect(() => seen.push(`keys=${Object.keys(o).join(',')}`))
watchSyncEffect(() => {
  let n = 0
  for (const k in o) n += 1
  seen.push(`forin=${n}`)
})
watchSyncEffect(() => seen.push(`spread=${Object.keys({...o}).length}`))
o.b = 2
delete o.a
trace('seen', seen)
