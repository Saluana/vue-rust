// Hoisted function declarations and labelled blocks inside an async body; a catch parameter destructured around an await.
function v(x) {
  return Promise.resolve(x)
}
export async function run() {
  trace('before', helper(2))
  const r = await v(helper(3))
  trace('after', r)
  block: {
    if (await v(true)) break block
    trace('unreached', 1)
  }
  try {
    throw { code: 7, label: 'seven' }
  } catch ({ code, label }) {
    trace('caught', [await v(code), label])
  }
  trace('done', 'ok')
  function helper(n) {
    return n * 10
  }
}
