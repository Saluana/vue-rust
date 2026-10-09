// @tier 3
// @covers L19
// The fake clock starts at 0 and moves only through advance; awaiting tick does not move it.
export async function run() {
  const t = testHost.timers
  const start = t.now()
  await testHost.tick()
  const afterTick = t.now()
  await t.advance(250)
  trace('clock', start, afterTick, t.now())
  await t.advance(0)
  trace('zero-advance', t.now())
}
