// Default exports: an anonymous function declaration (hoisted) and an expression.
import fn, { value as v } from './forms.js'
import anon from './anon.js'
export function run() {
  trace('fn', fn())
  trace('v', v)
  trace('anon', anon.tag)
}
