// A .vue module imported like any other: its default export is the component object (hand-written expected).
import Child from './Child.vue'
export function run() {
  trace('name', Child.name)
}
