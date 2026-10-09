function fn({ x = 1, y } = {}, { z = 9 } = {}, ...tail) {
  return `${x}|${y}|${z}|${tail.length}`
}
trace('params', fn(), fn({ y: 'why' }, { z: 'zz' }, 1, 2))
const { id = 0, name } = { name: 'n' }
trace('decl-pattern', id, name)
