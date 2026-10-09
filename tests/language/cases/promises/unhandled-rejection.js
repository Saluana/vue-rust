// @tier 3
// @covers L14
// Rejections still unhandled when the queue drains are listed in unhandledRejections, in rejection order.
Promise.reject(new Error('first lost'))
export async function run() {
  Promise.reject(new TypeError('second lost'))
  await testHost.tick()
  trace('run-done', true)
}
