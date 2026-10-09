// @tier 2
// @covers V14
// A cyclic object is traversed by a deep watch without looping. The watcher terminates and still sees nested writes.
import {reactive, watch} from 'vue'
const node = reactive({value: 1, next: null})
node.next = node
const log = []
watch(node, () => log.push('changed'), {deep: true})
export async function run() {
  node.value = 2
  await testHost.tick()
  trace('cycle', log, node.next.value)
}
