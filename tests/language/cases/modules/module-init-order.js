// @tier 1
// @covers L16
// Function declarations are initialized before module code runs; const is only usable after its line.
trace('step-1', typeof helper, helper())
function helper() {
  return 'hoisted'
}
const ready = 'after declaration'
trace('step-2', ready)
