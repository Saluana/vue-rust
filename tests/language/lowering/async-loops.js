// Await in loop tests, updates and bodies; break and continue (labelled too) across awaits; for-of and for-in bodies.
function v(x) {
  return Promise.resolve(x)
}
export async function run() {
  let i = 0
  while ((await v(i)) < 3) {
    i = (await v(i)) + 1
  }
  trace('while', i)
  let j = 0
  do {
    j += await v(2)
  } while (j < 7)
  trace('do', j)
  const acc = []
  for (let k = await v(0); k < 4; k = k + (await v(1))) {
    if (k === 1) continue
    acc.push(k)
  }
  trace('for', acc)
  let s = 0
  outer: for (const a of [1, 2, 3]) {
    for (const b of await v([10, 20, 30])) {
      if (b === 20) continue outer
      if (a === 3) break outer
      s += a * b + (await v(0))
    }
  }
  trace('nested', s)
  const keys = []
  for (const key in { p: 1, q: 2 }) {
    keys.push(await v(key))
  }
  trace('forin', keys)
}
