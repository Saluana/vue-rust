// @tier 2
// @covers V14
// An array of sources calls back once per flush with one value per source. Old values come in the same shape.
import {ref, watch} from 'vue'
const a = ref(1)
const b = ref('x')
const log = []
watch([a, () => `${b.value}!`], ([na, nb], [oa, ob]) => log.push([na, nb, oa, ob]))
export async function run() {
  a.value = 2
  b.value = 'y'
  await testHost.tick()
  trace('log', log)
}
