// @tier 3
// @covers L19
// The job queue is drained after each timer callback, so promise jobs run before the next timer fires.
const log = []
export async function run() {
  const t = testHost.timers
  t.setTimeout(() => {
    log.push('timer-1')
    Promise.resolve().then(() => log.push('micro-after-1'))
  }, 0)
  t.setTimeout(() => log.push('timer-2'), 0)
  await t.advance(0)
  trace('between', log)
}
