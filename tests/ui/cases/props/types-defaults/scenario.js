// @tier 1
// @covers U11
// Defaults: literal, factory and function defaults apply when a prop is absent. An empty string passed to a Boolean prop casts to true.
import Types from './Types.vue'
export function run() {
  trace('absent', testHost.find(testHost.mount(Types), '#p').text())
  trace('present', testHost.find(testHost.mount(Types, { flag: '', count: 5, label: undefined }), '#p').text())
}
