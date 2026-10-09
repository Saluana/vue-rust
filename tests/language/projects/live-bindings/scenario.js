// An importer sees the exporter's current value (live binding), also through a namespace object.
import { count, bump, read } from './counter.js'
import * as ns from './counter.js'
export function run() {
  trace('start', count)
  trace('read0', read())
  bump()
  bump()
  trace('after', count)
  trace('ns', ns.count)
  trace('read2', read())
}
