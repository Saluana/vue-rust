export default 1 + 2
trace('default-init', 'ran')
export function run() {
  trace('run-after-default', true)
}
