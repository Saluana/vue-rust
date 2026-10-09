// @tier 1
// @covers V08
// toRef on a reactive property is linked both ways: a write through either side is seen by the other and by effects.
import {reactive, toRef, watchSyncEffect} from 'vue'
const o = reactive({a: 1, b: 2})
const ra = toRef(o, 'a')
let runs = 0
watchSyncEffect(() => {
  ra.value
  runs += 1
})
ra.value = 5
o.a = 6
trace('alias', o.a, ra.value, runs)
