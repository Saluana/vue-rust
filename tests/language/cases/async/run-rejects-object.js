// @tier 3
// @covers L15
// Any value can be the reason of a rejected run(); here a plain object reason built from the fake clock.
export async function run() {
  await testHost.timers.advance(10)
  trace('after-advance', testHost.timers.now())
  throw { kind: 'timeout', at: testHost.timers.now() }
}
