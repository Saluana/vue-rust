// Parameters with defaults and rest, a named async function expression that calls itself, methods using this.
function v(x) {
  return Promise.resolve(x)
}
const named = async function self(n) {
  if (n <= 0) return 'done'
  return await self(n - 1)
}
export async function run() {
  trace('named', await named(3))
  const o = {
    v: 5,
    async get(extra = 1, ...more) {
      return this.v + extra + more.length + (await v(this.v))
    },
  }
  trace('method', await o.get())
  trace('method2', await o.get(2, 'a', 'b'))
  const holder = { k: 7, run() { return (async () => this.k + (await v(1)))() } }
  trace('arrow', await holder.run())
}
