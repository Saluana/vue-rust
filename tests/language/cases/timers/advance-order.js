// @tier 3
// @covers L19
// Timers fire in due-time order, ties by creation order; during a callback the clock reads that timer's due time.
const log = []
export async function run() {
  const t = testHost.timers
  t.setTimeout(() => log.push(`late@${t.now()}`), 20)
  t.setTimeout(() => log.push(`tie-first@${t.now()}`), 10)
  t.setTimeout(() => log.push(`tie-second@${t.now()}`), 10)
  t.setTimeout(() => log.push(`early@${t.now()}`), 5)
  await t.advance(30)
  trace('fired', log, t.now())
}
