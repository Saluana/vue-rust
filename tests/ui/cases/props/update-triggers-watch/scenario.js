// @tier 2
// @covers U11,V14
// A parent update changes a prop; a watcher in the child sees the old and new values, and nothing runs when the prop is unchanged.
import Watched from './Watched.vue'
export async function run() {
  const m = testHost.mount(Watched, { n: 1, log: (...v) => trace('log', ...v) })
  m.setProps({ n: 2, log: (...v) => trace('log', ...v) })
  await testHost.tick()
  m.setProps({ n: 2, log: (...v) => trace('log', ...v) })
  await testHost.tick()
  trace('text', testHost.find(m, '#p').text())
}
