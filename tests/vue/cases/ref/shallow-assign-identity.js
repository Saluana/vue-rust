// @tier 1
// @covers V02
// shallowRef stores the object it is given: identity is kept, and assigning a new object is a change.
import {shallowRef, watchSyncEffect} from 'vue'
const a = {tag: 'a'}
const b = {tag: 'b'}
const s = shallowRef(a)
let runs = 0
watchSyncEffect(() => {
  s.value
  runs += 1
})
trace('same-object', s.value === a, runs)
s.value = b
s.value = b
trace('after', s.value === b, runs)
