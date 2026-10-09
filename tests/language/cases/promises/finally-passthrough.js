// @tier 3
// @covers L14
// finally calls its callback with no argument and passes the outcome through, unless the callback throws.
export async function run() {
  const a = await Promise.resolve('value').finally(() => trace('finally-ok'))
  trace('passed', a)
  try {
    await Promise.reject(new Error('original')).finally(() => {
      throw new Error('replaced')
    })
  } catch (e) {
    trace('replaced-by', e.message)
  }
}
