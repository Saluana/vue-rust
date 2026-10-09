// @tier 2
// @covers V20
// A failing render leaves a comment where the component's output was, and an update that fixes the render restores it.
import Child from './Child.vue'
export async function run() {
  const m = testHost.mount(Child, { bad: false }, { errorHandler: (err, inst, info) => trace('app', err.message, info) })
  trace('good', m.html())
  m.setProps({ bad: true })
  await testHost.tick()
  trace('bad', m.html())
  m.setProps({ bad: false })
  await testHost.tick()
  trace('recovered', m.html())
}
