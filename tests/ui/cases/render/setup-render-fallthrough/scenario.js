// @tier 2
// @covers A10,A11,U13
// Attributes that a parent passes to a render-function component reach its attrs. With inheritAttrs left on, class, attributes and listeners are merged onto the render root; with inheritAttrs false the root keeps only its own class.
import Box from './Box.vue'
import Quiet from './Quiet.vue'
export async function run() {
  const log = []
  const m = testHost.mount(Box, { kind: 'a', class: 'outer', title: 'T', onPing: (ev) => log.push(['ping', ev.type]) })
  trace('box-html', m.html())
  testHost.dispatch(testHost.find(m, 'div'), 'ping')
  await testHost.tick()
  trace('box-listener', log)
  const q = testHost.mount(Quiet, { title: 'Q', class: 'outer' })
  trace('quiet-html', q.html())
  m.setProps({ kind: 'b', class: 'outer' })
  await testHost.tick()
  trace('box-after-setProps', m.html())
}
