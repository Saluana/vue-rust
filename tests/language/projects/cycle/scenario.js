// A module cycle: even.js and odd.js import each other; function declarations are visible before evaluation.
import { isEven } from './even.js'
import { isOdd, label } from './odd.js'
export function run() {
  trace('even4', isEven(4))
  trace('odd3', isOdd(3))
  trace('label', label())
}
