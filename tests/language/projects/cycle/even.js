import { isOdd } from './odd.js'
export function isEven(n) {
  return n === 0 ? true : isOdd(n - 1)
}
export const LABEL = 'even'
