// @tier 3
// @covers L14
// A rejection handled before the final check is not reported, even if its first drain saw no handler.
const late = Promise.reject(new Error('handled later'))
export async function run() {
  await testHost.timers.advance(0)
  late.catch((e) => trace('handled', e.message))
  await testHost.tick()
}
