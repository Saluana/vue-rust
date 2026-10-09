// Spread arguments and elements evaluated around awaits (argument and element lists live in frame accumulators).
function v(x) {
  return Promise.resolve(x)
}
function collect(...xs) {
  return xs.length + ':' + xs.join(',')
}
export async function run() {
  const xs = [1, 2]
  trace('spread', collect(...xs, await v(3), ...(await v([4, 5]))))
  trace('arr-spread', [...xs, await v(9), , ...[7]])
  const o = { ...(await v({ p: 1 })), q: await v(2) }
  trace('objspread', o)
}
