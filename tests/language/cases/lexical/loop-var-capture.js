// @tier 2
// @covers L01,L02
// A var loop counter is shared by every closure created in it.
const fns = []
for (var i = 0; i < 3; i++) {
  fns.push(() => i)
}
trace('var-captures', fns.map((f) => f()), i)
