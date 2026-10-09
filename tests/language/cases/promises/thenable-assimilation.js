// @tier 3
// @covers L14
// Resolving with a thenable schedules a job that calls its then method; the adopted value arrives later.
const log = []
const thenable = {
  then(resolve) {
    log.push('then called')
    resolve('from-thenable')
  },
}
export async function run() {
  const p = Promise.resolve(thenable)
  log.push('after resolve')
  const v = await p
  log.push(`value ${v}`)
  await testHost.tick()
  trace('thenable', log)
}
