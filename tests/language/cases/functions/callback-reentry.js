// @tier 2
// @covers L02
// A callback re-enters the function that called it; the nested run completes before the outer loop continues.
const events = []
function run(items, cb) {
  for (const it of items) {
    events.push(`start ${it}`)
    cb(it)
    events.push(`end ${it}`)
  }
}
let depth = 0
run(['x', 'y'], (item) => {
  depth += 1
  events.push(`cb ${item} depth ${depth}`)
  if (item === 'x') run(['z'], (inner) => events.push(`inner ${inner}`))
  depth -= 1
})
trace('events', events, depth)
