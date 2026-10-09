// @tier 2
// @covers V14,V17
// A default (pre) watcher runs before the component re-renders, so it runs before updated.
import Child from './Child.vue'
export async function run() {
  const log = (v) => trace('log', v)
  const m = testHost.mount(Child, { n: 1, log })
  m.setProps({ n: 2, log })
  await testHost.tick()
  trace('html', m.html())
}
