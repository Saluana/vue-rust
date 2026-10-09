// @tier 2
// @covers V14
// A ref holding an object, watched with deep: true, calls back on nested writes. There, old and new are the same object.
import {ref, watch} from 'vue'
const r = ref({n: 1})
const log = []
watch(r, (v, old) => log.push(v === old), {deep: true})
export async function run() {
  r.value.n = 2
  await testHost.tick()
  r.value = {n: 3}
  await testHost.tick()
  trace('identity', log)
}
