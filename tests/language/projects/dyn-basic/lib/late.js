// Reached only through import(): evaluated by the first import, after its own dependency.
import { base } from './base.js'
trace('late-eval')
export const value = base + 1
export let count = 0
export function bump() {
  count = count + 1
}
