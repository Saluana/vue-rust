// @tier 1
// @covers U16
// Nested v-for: the inner loop reads the outer item's cells, and the output keeps the nesting.
import Grid from './Grid.vue'
export function run() {
  const rows = [{ id: 'r1', cells: ['a', 'b'] }, { id: 'r2', cells: ['c'] }]
  trace('html', testHost.mount(Grid, { rows }).html())
}
