// @tier 1
// @covers V03
// push, pop, shift and unshift each rerun a length reader once per call, and the mutators do not re-enter their own effect.
import {reactive, watchSyncEffect} from 'vue'
const list = reactive([1])
let runs = 0
watchSyncEffect(() => {
  list.length
  runs += 1
})
list.push(2)
trace('after-push', runs)
list.pop()
trace('after-pop', runs)
list.unshift(0)
trace('after-unshift', runs)
list.shift()
trace('after-shift', runs, list.length)
