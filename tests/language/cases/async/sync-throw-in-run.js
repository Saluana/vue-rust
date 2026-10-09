// @tier 3
// @covers L15
// A run() that is not async and throws synchronously is also recorded as result "throw".
export function run() {
  trace('before-sync-throw', 'yes')
  throw 'sync value'
}
