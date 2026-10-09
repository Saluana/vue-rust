// @tier 2
// @covers V18
// A state change inside a child re-renders only that child: the parent's updated hook does not run.
import Host from './Host.vue'
export async function run() {
  const m = testHost.mount(Host, { log: (v) => trace('log', v) })
  testHost.click(testHost.find(m, '#b'))
  await testHost.tick()
  trace('html', m.html())
}
