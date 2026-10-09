// @tier 3
// @covers L15
// finally runs after the awaited value settles, on both normal return and a throw after await.
const log = []
async function job(fail) {
  try {
    log.push('start')
    await Promise.resolve(1)
    if (fail) throw new Error('job failed')
    log.push('ok')
    return 'done'
  } finally {
    log.push(fail ? 'cleanup-fail' : 'cleanup-ok')
  }
}
export async function run() {
  trace('result', await job(false))
  const reason = await job(true).catch((e) => e.message)
  trace('rejected', reason)
  trace('log', log)
}
