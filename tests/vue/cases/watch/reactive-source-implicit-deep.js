// @tier 2
// @covers V14
// A reactive object as the source is watched deeply. A nested write calls back with the same object as old and new.
import {reactive, watch} from 'vue'
const state = reactive({user: {name: 'a'}, tags: []})
const log = []
watch(state, (v, old) => log.push([v === old, v.user.name]))
export async function run() {
  state.user.name = 'b'
  await testHost.tick()
  state.tags.push('x')
  await testHost.tick()
  trace('log', log)
}
