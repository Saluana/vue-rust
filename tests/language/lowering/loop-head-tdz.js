export async function run() {
let x = [1]
try { for (let x of x) trace('bad-of', x) } catch (e) { trace('of', e.name) }
let y = { a: 1 }
try { for (let y in y) trace('bad-in', y) } catch (e) { trace('in', e.name) }
async function check() {
  try { for (let x of x) trace('bad-async-of', x) } catch (e) { trace('async-of', e.name) }
  try { for (let y in y) trace('bad-async-in', y) } catch (e) { trace('async-in', e.name) }
}
await check()
let get
for (let n of (get = () => n, [1, 2])) {
  try { trace('bad-rhs-closure', get()) } catch (e) { trace('rhs-closure', e.name) }
}
async function capturedHead() {
  let get
  for (let n of (get = () => n, [1, 2])) {
    await 0
    try { trace('bad-async-rhs-closure', get()) } catch (e) { trace('async-rhs-closure', e.name) }
  }
}
await capturedHead()
const read = []
for (const n of [1, 2]) read.push(() => n)
trace('fresh-iterations', read.map(f => f()))
}
