// try/catch/finally with awaits: a return or a throw passes through finally after its awaits; finally can override.
function v(x) {
  return Promise.resolve(x)
}
async function early(log) {
  try {
    log.push('try')
    return await v('from-try')
  } finally {
    log.push(await v('finally'))
  }
}
async function overriding() {
  try {
    return 'lost'
  } finally {
    await v(0)
    return 'finally-wins'
  }
}
async function thrower(log) {
  try {
    await v(1)
    throw new Error('boom')
  } catch (e) {
    log.push('caught ' + e.message)
    throw new Error('rethrown')
  } finally {
    log.push('finally-after-throw')
  }
}
export async function run() {
  const log = []
  trace('early', await early(log))
  trace('overriding', await overriding())
  try {
    await thrower(log)
  } catch (e) {
    trace('outer', e.message)
  }
  trace('log', log)
  let n = 0
  for (const x of [1, 2, 3, 4]) {
    try {
      if (x === 2) continue
      if (x === 4) break
      n += await v(x)
    } finally {
      n += 100
    }
  }
  trace('loop-finally', n)
}
