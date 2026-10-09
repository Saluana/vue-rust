// @tier 3
// @covers L14
// Reactions run as microtasks in registration order; a chained then runs after the step it depends on.
const log = []
const p = Promise.resolve('start')
p.then((v) => log.push(`a1 ${v}`)).then(() => log.push('a2'))
p.then((v) => log.push(`b1 ${v}`)).then(() => log.push('b2'))
log.push('sync')
export async function run() {
  await testHost.tick()
  trace('order', log)
}
