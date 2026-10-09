// @tier 1
// @covers L13
// A module that throws while evaluating is recorded as result "throw" with the thrown value; run() is never called.
trace('before-throw', 1)
export function run() {
  trace('run-called', true)
}
throw 'module failed'
