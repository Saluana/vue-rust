// @tier 1
// @covers V24
// provide/inject: the nearest provider above the injector wins, so the middle component's value is found.
import Outer from './Outer.vue'
export function run() {
  trace('html', testHost.mount(Outer).html())
}
