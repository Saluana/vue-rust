// @tier 2
// @covers U07
// A textarea keeps line breaks in the model; its content shows in the snapshot as a dirty value prop, not as text children.
import Text from './Text.vue'
export async function run() {
  const m = testHost.mount(Text)
  testHost.input(testHost.find(m, '#t'), 'a\nb')
  await testHost.tick()
  trace('length', testHost.find(m, '#out').text())
  trace('snapshot', m.snapshot())
}
