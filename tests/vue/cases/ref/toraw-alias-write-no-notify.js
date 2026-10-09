// @tier 1
// @covers V10
// A write through the raw object (toRaw) bypasses reactivity: effects do not rerun. Writes through the proxy do.
import {reactive, toRaw, watchSyncEffect} from 'vue'
const o = reactive({n: 1})
const raw = toRaw(o)
let runs = 0
watchSyncEffect(() => {
  o.n
  runs += 1
})
raw.n = 2
trace('raw-write', runs, o.n, raw.n)
o.n = 3
trace('proxy-write', runs, o.n, raw.n)
