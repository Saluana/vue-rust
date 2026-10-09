// Named, default and namespace imports from relative modules; a renamed import; namespace keys in code-unit order.
import greet, { answer, origin } from './lib/point.js'
import * as util from './lib/util.js'
import { twice as double } from './lib/util.js'
export function run() {
  trace('answer', answer)
  trace('greet', greet('Ada'))
  trace('origin', origin.x + origin.y)
  trace('double', double(21))
  trace('util', util.twice(4) + util.KIND.length)
  trace('keys', Object.keys(util).join(','))
}
