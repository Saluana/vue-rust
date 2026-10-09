// @tier 1
// @covers U11,V29
// An emit during setup reaches the parent's listener before the first render is complete.
import Setup from './Setup.vue'
export function run() {
  trace('before-mount')
  const m = testHost.mount(Setup, { onReady: (v) => trace('ready', v) })
  trace('after-mount', testHost.find(m, '#p').text())
}
