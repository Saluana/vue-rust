// @tier 2
// @covers V06,V14
// A computed used as a watch source calls back only when the computed value changes, not on every write it depends on.
import {computed, ref, watch} from 'vue'
const n = ref(1)
const parity = computed(() => n.value % 2)
const log = []
const stop = watch(parity, (v, old) => log.push([v, old]))
export async function run() {
  n.value = 3
  await testHost.tick()
  trace('same-parity', log)
  n.value = 4
  await testHost.tick()
  trace('changed-parity', log)
  stop()
}
