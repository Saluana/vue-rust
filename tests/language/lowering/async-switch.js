// switch with awaits in the discriminant, in case tests and in bodies; fallthrough and default.
function v(x) {
  return Promise.resolve(x)
}
export async function run() {
  const out = []
  for (const k of ['a', 'b', 'c', 'z']) {
    switch (await v(k)) {
      case await v('a'):
        out.push('A')
      case 'b':
        out.push('B' + (await v(1)))
        break
      case await v('c'):
        out.push('C')
        break
      default:
        out.push('D')
    }
  }
  trace('switch', out)
}
