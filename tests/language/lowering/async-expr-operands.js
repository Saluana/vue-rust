// Await in operand positions: operands are evaluated left to right, and values computed before an await survive it.
function v(x) {
  return Promise.resolve(x)
}
export async function run() {
  const obj = { a: 1, b: await v(2), [await v('k')]: await v(3) }
  trace('obj', obj)
  const arr = [1, await v(2), , await v(4)]
  trace('arr-len', arr.length)
  trace('arr-hole', 2 in arr)
  let x = 10
  x += await v(5)
  trace('x', x)
  trace('sum', (await v(1)) + (await v(2)) * (await v(3)))
  trace('tmpl', `t-${await v('a')}-${await v(0)}`)
  trace('or', (await v(0)) || (await v('fallback')))
  trace('nullish', (await v(null)) ?? (await v('nn')))
  trace('cond', (await v(true)) ? await v('yes') : await v('no'))
  const member = { deep: { list: [10, 20] } }
  trace('member', member.deep.list[await v(1)])
  trace('call', Math.max(await v(3), await v(9)))
}
