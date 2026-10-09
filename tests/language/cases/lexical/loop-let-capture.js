// @tier 2
// @covers L01,L02
// Each iteration of a for-let or for-of loop gets a fresh binding, so closures keep their own value.
const fns = []
for (let i = 0; i < 3; i++) {
  fns.push(() => i)
}
trace('let-captures', fns.map((f) => f()))
const seen = []
for (const k of ['a', 'b']) {
  seen.push(() => k)
}
trace('for-of-captures', seen.map((f) => f()))
