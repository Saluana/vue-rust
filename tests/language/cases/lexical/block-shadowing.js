// @tier 1
// @covers L01
// let and const are block scoped; inner declarations shadow outer ones without changing them.
let x = 'outer'
{
  let x = 'block'
  trace('inner', x)
  {
    const x = 'deeper'
    trace('deepest', x)
  }
  trace('inner-again', x)
}
trace('outer', x)
for (let i = 0; i < 2; i++) {
  let x = i * 10
  trace('loop', x)
}
trace('after-loop', x)
