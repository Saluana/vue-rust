// Await in if conditions, return await, optional chains with awaited keys and while conditions.
function v(x) {
  return Promise.resolve(x)
}
async function f(n) {
  if (await v(n > 1)) {
    return await v('big')
  } else {
    return 'small'
  }
}
export async function run() {
  trace('f', [await f(1), await f(2)])
  const holder = { inner: { key: 'k' } }
  trace('opt', holder?.inner?.[await v('key')])
  trace('nullish', holder.missing?.[await v('key')])
  let count = 0
  while (await v(count < 2)) count++
  trace('count', count)
}
