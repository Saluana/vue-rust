// Re-exports: named re-export with a rename, export *, export * as ns, and export { default } from.
import { first, second, all, ns as shapes } from './barrel.js'
import def from './barrel.js'
export function run() {
  trace('first', first)
  trace('second', second())
  trace('all', all)
  trace('shapes', shapes.square(3))
  trace('def', def)
}
