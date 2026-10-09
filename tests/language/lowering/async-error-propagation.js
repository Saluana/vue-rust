// A rejected await throws at the await site; catch sees the reason; an await inside an argument list can throw too.
function v(x) {
  return Promise.resolve(x)
}
async function fails(tag) {
  await v(tag)
  throw tag
}
export async function run() {
  const seen = []
  for (const t of ['x', 'y']) {
    try {
      await fails(t)
    } catch (e) {
      seen.push(e)
    }
  }
  trace('seen', seen)
  const results = []
  try {
    results.push(await v(1), await fails('z'), await v(3))
  } catch (e) {
    results.push('caught:' + e)
  }
  trace('results', results)
}
