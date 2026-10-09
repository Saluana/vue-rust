// @tier 3
// @covers L19
// clearTimeout cancels a pending timer; clearing an unknown id does nothing.
const log = []
export async function run() {
  const t = testHost.timers
  const a = t.setTimeout(() => log.push('a'), 5)
  const b = t.setTimeout(() => log.push('b'), 6)
  t.clearTimeout(a)
  t.clearTimeout(999)
  await t.advance(10)
  t.clearTimeout(b)
  trace('cleared', log, a < b, t.now())
}
