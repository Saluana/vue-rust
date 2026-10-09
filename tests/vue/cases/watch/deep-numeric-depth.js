// @tier 2
// @covers V14
// A numeric deep option limits the traversal. Writes at depth 3 (state.a.b.c) are seen by neither watcher here, a write
// at depth 2 (state.a.b) only by deep: 2, and a write at depth 1 (state.a) by both.
import {reactive, watch} from 'vue'
const state = reactive({a: {b: {c: 1}}})
const shallowLog = []
const deepLog = []
watch(state, () => shallowLog.push('d1'), {deep: 1})
watch(state, () => deepLog.push('d2'), {deep: 2})
export async function run() {
  state.a.b.c = 2
  await testHost.tick()
  trace('depth-3-write', shallowLog, deepLog)
  state.a.b = {c: 5}
  await testHost.tick()
  trace('depth-2-write', shallowLog, deepLog)
  state.a = {b: {c: 9}}
  await testHost.tick()
  trace('depth-1-write', shallowLog, deepLog)
}
