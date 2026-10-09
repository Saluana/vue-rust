// @tier 2
// @covers U17
// A 1000-row keyed list reversed in one update: the first and last rows swap places and the count is unchanged.
import Big from './Big.vue'
export async function run() {
  const rows = []
  for (let i = 0; i < 1000; i++) rows.push(i)
  const m = testHost.mount(Big, { rows })
  m.setProps({ rows: rows.slice().reverse() })
  await testHost.tick()
  trace('count', testHost.findAll(m, 'p').length)
  trace('first', testHost.find(m, 'p:nth-child(1)').text(), testHost.find(m, 'p:nth-child(1000)').text())
}
