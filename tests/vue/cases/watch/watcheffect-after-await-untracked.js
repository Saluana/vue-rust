// @tier 3
// @covers V13
// watchEffect tracks only the reads made before its first await. A dependency read after the await is not tracked.
import {ref, watchEffect} from 'vue'
const early = ref(1)
const late = ref(1)
let runs = 0
watchEffect(async () => {
  runs += 1
  early.value
  await null
  late.value
})
export async function run() {
  await testHost.tick()
  late.value = 2
  await testHost.tick()
  const before = runs
  early.value = 2
  await testHost.tick()
  trace('runs', before, runs)
}
