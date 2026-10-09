// @tier 3
// @covers L15
// Module evaluation and its job drain finish before run() starts, so run() sees the module's queued results.
const state = []
state.push('module-init')
Promise.resolve().then(() => state.push('module-microtask'))
export async function run() {
  state.push('run-start')
  await testHost.tick()
  trace('order', state)
}
