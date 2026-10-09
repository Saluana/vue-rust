// @tier 2
// @covers V02,V14
// A watch on a shallowRef ignores nested writes. triggerRef forces a callback even though the value is the same object.
import {shallowRef, triggerRef, watch} from 'vue'
const s = shallowRef({n: 1})
const log = []
watch(s, (v, old) => log.push(v === old))
export async function run() {
  s.value.n = 2
  await testHost.tick()
  trace('nested', log)
  triggerRef(s)
  await testHost.tick()
  trace('forced', log)
}
