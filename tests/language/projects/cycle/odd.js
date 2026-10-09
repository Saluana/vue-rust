import { isEven, LABEL } from './even.js'
export function isOdd(n) {
  return n === 0 ? false : isEven(n - 1)
}
export function label() {
  return LABEL
}
