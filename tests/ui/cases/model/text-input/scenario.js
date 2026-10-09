// @tier 2
// @covers U07,U08
// v-model on text inputs: input updates at once, .trim strips the ends, .lazy waits for change, .number parses or keeps the string.
import TextModel from './TextModel.vue'
export async function run() {
  const m = testHost.mount(TextModel)
  testHost.input(testHost.find(m, '#t'), 'hello')
  testHost.input(testHost.find(m, '#tr'), '  padded  ')
  testHost.input(testHost.find(m, '#lz'), 'lazy')
  testHost.input(testHost.find(m, '#nm'), '12.5')
  await testHost.tick()
  trace('after-input', m.html())
  testHost.change(testHost.find(m, '#lz'), 'lazy-done')
  testHost.input(testHost.find(m, '#nm'), 'abc')
  await testHost.tick()
  trace('after-change', m.html())
}
