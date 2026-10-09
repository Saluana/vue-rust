// @tier 2
// @covers V13,V16
// watchEffect runs its cleanup before each re-run and once more when the component unmounts.
import Eff from './Eff.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const m = testHost.mount(Eff, { n: 1, log })
  m.setProps({ n: 2, log })
  await testHost.tick()
  m.unmount()
  trace('unmounted')
}
