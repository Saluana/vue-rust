// @tier 2
// @covers V22,U21
// Hooks of a kept-alive component: mounted once, then deactivated and activated; unmounting the app unmounts it.
import A from './A.vue'
import Hooks from './Hooks.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const m = testHost.mount(Hooks, { current: A, log })
  m.setProps({ current: null, log })
  await testHost.tick()
  m.setProps({ current: A, log })
  await testHost.tick()
  m.unmount()
  trace('done')
}
