// @tier 1
// @covers V03
// Deep tracking follows the current path. Replacing a nested object reruns readers, and the old object no longer does.
import {reactive, watchSyncEffect} from 'vue'
const state = reactive({child: {v: 1}})
const old = state.child
let runs = 0
watchSyncEffect(() => {
  state.child.v
  runs += 1
})
state.child = {v: 2}
const afterReplace = runs
old.v = 99
trace('replace', afterReplace, runs, state.child.v)
state.child.v = 3
trace('new-path', runs, state.child.v)
