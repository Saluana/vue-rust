trace('step-1', typeof helper, helper())
function helper() {
  return 'hoisted'
}
const ready = 'after declaration'
trace('step-2', ready)
let late
trace('late-undefined', late)
late = 'set'
trace('late-set', late)
