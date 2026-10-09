trace('before-run', 1)
export function run() {
  trace('inside-run', 2)
  throw { reason: 'run' }
}
