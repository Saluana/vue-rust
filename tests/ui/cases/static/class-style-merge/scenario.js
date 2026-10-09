// @tier 2
// @covers U03,U13
// class and style bindings: arrays and objects merge with the static attribute; an empty style value removes its property.
import Merge from './Merge.vue'
export async function run() {
  const m = testHost.mount(Merge, { on: false, extra: undefined })
  trace('off', m.html())
  m.setProps({ on: true, extra: 'd' })
  await testHost.tick()
  trace('on', m.html())
  trace('class', testHost.find(m, 'span').attr('class'))
}
