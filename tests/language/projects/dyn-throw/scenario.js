// Dynamic import (WP-C1c): a module that throws while the first import evaluates it rejects that import. Later imports
// of the module, and of a module that imports it statically, reject with the same error value.
export async function run() {
  const first = await import('./lib/bad.js').then(() => 'resolved', (e) => e)
  trace('first', first.name, first.message)
  const second = await import('./lib/bad.js').then(() => 'resolved', (e) => e)
  trace('second-same', second === first)
  const parent = await import('./lib/bad-parent.js').then(() => 'resolved', (e) => e)
  trace('parent-same', parent === first)
}
