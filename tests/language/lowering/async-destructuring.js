// Destructuring with awaits in defaults, computed keys, holes and rest elements, in declarations and assignments.
function v(x) {
  return Promise.resolve(x)
}
export async function run() {
  const { a = await v(1), b: { c = await v(2) } = {}, ...rest } = { b: {}, z: 9, y: 8 }
  trace('obj', [a, c, rest.z, rest.y])
  const [p = await v('p'), , q = await v('q'), ...tail] = [undefined, 'skip', await v(undefined), 4, 5]
  trace('arr', [p, q, tail.length, tail[1]])
  let m
  let n
  ;[m = await v('m'), n] = [undefined, await v('n')]
  trace('assign', [m, n])
  const key = 'dyn'
  const { [key]: dv = await v('dflt'), ...others } = { other: 1 }
  trace('computed', [dv, others.other])
  const { length } = await v('four')
  trace('length', length)
}
