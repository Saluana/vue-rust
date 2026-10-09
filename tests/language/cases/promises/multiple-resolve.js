// @tier 3
// @covers L14
// Only the first resolve or reject takes effect; resolving with a promise adopts it and ignores later calls.
export async function run() {
  const p = new Promise((resolve, reject) => {
    resolve('first')
    resolve('second')
    reject(new Error('ignored'))
  })
  const v = await p
  const q = new Promise((resolve) => {
    resolve(Promise.resolve('inner'))
    resolve('outer')
  })
  trace('multi', v, await q)
}
