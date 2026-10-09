// @tier 1
// @covers U11
// A Boolean prop given the string "false" keeps the string (it is not cast to false); only an absent value becomes false.
import Flag from './Flag.vue'
export function run() {
  trace('string', testHost.find(testHost.mount(Flag, { flag: 'false' }), '#p').text())
  trace('absent', testHost.find(testHost.mount(Flag), '#p').text())
}
