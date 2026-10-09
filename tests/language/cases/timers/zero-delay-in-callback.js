// @tier 3
// @covers L19
// A zero-delay timer created inside a callback is due at once; it runs in the same advance, after timers already due.
const log = []
export async function run() {
  const t = testHost.timers
  t.setTimeout(() => {
    log.push(`outer@${t.now()}`)
    t.setTimeout(() => log.push(`inner@${t.now()}`), 0)
    t.setTimeout(() => log.push(`later@${t.now()}`), 5)
  }, 2)
  t.setTimeout(() => log.push(`sibling@${t.now()}`), 2)
  await t.advance(3)
  trace('zero-delay', log, t.now())
  await t.advance(10)
  trace('after-second-advance', log.length, t.now())
}
