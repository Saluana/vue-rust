import { getB } from './cyc-b.js'
trace('cyc-a-eval', getB())
export const name = 'A'
export function getA() {
  return name
}
