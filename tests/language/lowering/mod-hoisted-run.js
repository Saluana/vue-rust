trace('before-decl', typeof run)
export function run() {
  trace('run', helper())
}
function helper() {
  return 'h'
}
