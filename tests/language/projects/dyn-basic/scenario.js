// Dynamic import (WP-C1c): a module reached only through import() is evaluated by the first import, not at startup.
// Repeated imports resolve to the same namespace object, the namespace reads live bindings, and a static import of
// the same module gives the same object.
import * as early from './lib/early.js'
trace('startup')
export async function run() {
  trace('run')
  const pending = import('./lib/late.js')
  trace('called')
  const ns = await pending
  trace('value', ns.value)
  trace('again', ns === (await import('./lib/late.js')))
  trace('static-same', (await import('./lib/early.js')) === early)
  ns.bump()
  trace('live', ns.count)
  trace('keys', Object.keys(ns))
  trace('tag', ns[Symbol.toStringTag])
  trace('extensible', Object.isExtensible(ns))
}
