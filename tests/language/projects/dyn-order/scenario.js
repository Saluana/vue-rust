// Dynamic import (WP-C1c) among other promise jobs and awaits. The project oracle runs Node with loader hooks, and each
// round trip of its loader runs only after the promise jobs queued before it. So the chain and the awaits finish before
// the leaf is evaluated, and the import with a dependency is evaluated after the leaf, once its own loads are done.
export async function run() {
  const leaf = import('./order/leaf.js')
  const deps = import('./order/deps.js')
  let chain = Promise.resolve()
  for (let i = 1; i <= 24; i++) chain = chain.then(() => trace('then', i))
  const steps = (async () => {
    for (let i = 1; i <= 12; i++) {
      await null
      trace('await', i)
    }
  })()
  const ns = await leaf
  trace('leaf', ns.value)
  trace('deps', (await deps).total)
  await chain
  await steps
  trace('again', (await import('./order/leaf.js')) === ns)
}
