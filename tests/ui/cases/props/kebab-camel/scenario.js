// @tier 1
// @covers U11
// A camelCase prop accepts the kebab-case attribute form; the prop value is the same either way.
import User from './User.vue'
export function run() {
  trace('camel', testHost.find(testHost.mount(User, { userName: 'a' }), '#u').text())
  trace('kebab', testHost.find(testHost.mount(User, { 'user-name': 'b' }), '#u').text())
}
