// @tier 3
// @covers L15
// A rejected run() is recorded as result "throw" with its reason; entries traced before the rejection are kept.
trace('run-start', true)
export async function run() {
  trace('inside', 1)
  await testHost.tick()
  throw new Error('run failed')
}
