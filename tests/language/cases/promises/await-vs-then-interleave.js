// @tier 3
// @covers L14,L15
// await and then share one microtask queue; each await or then adds a job in the order it was queued.
const log = []
async function asyncStep() {
  log.push('async-start')
  await null
  log.push('async-after-await')
  await null
  log.push('async-after-second-await')
}
export async function run() {
  const first = asyncStep()
  Promise.resolve().then(() => log.push('then-1'))
  log.push('sync-after-call')
  await first
  log.push('run-after-first')
  await testHost.tick()
  trace('interleave', log)
}
