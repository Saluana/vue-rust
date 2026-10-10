// Dynamic import inside a static cycle (WP-C1c): cyc-a and cyc-b import each other statically. cyc-b starts an import of
// cyc-a while the cycle is still being evaluated; that import settles once the cycle has finished. run() awaits it before
// tracing anything: under the project oracle (loader hooks) the settlement of this import falls before or after run()
// starts, depending on the event loop, so a trace made before the await would race with it.
import { getA } from './cycle/cyc-a.js'
import { pending } from './cycle/cyc-b.js'
export async function run() {
  const ns = await pending
  trace('static getA', getA())
  trace('pending', ns.name, ns.getA())
  trace('same', (await import('./cycle/cyc-a.js')) === ns)
  trace('keys', Object.keys(ns))
}
