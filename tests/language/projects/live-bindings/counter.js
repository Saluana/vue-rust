export let count = 0
export function bump() {
  count = count + 1
}
export function read() {
  return count
}
