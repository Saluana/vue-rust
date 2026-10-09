// @tier 1
// @covers V01
// Object.is decides a change for objects too: an equal-looking new object is a change, the same object is not.
import {ref, watchSyncEffect} from 'vue'
const r = ref({k: 1})
let runs = 0
watchSyncEffect(() => {
  r.value
  runs += 1
})
r.value = {k: 1}
r.value = r.value
const same = r.value
r.value = same
trace('runs', runs)
