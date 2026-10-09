// Closures created inside async bodies capture cells that live in the frame; per-iteration bindings survive awaits;
// async arrows and methods share the same entry.
function v(x) {
  return Promise.resolve(x)
}
export async function run() {
  const fns = []
  for (let i = 0; i < 3; i++) {
    const base = await v(i * 10)
    fns.push(() => base + i)
  }
  trace('closures', [fns[0](), fns[1](), fns[2]()])
  let counter = 0
  const inc = async () => {
    counter = counter + (await v(1))
    return counter
  }
  trace('inc1', await inc())
  trace('inc2', await inc())
  trace('counter', counter)
  const obj = {
    name: 'o',
    async greet(suffix) {
      const parts = [this.name, await v(suffix)]
      return parts.join('-')
    },
  }
  trace('greet', await obj.greet('hi'))
}
