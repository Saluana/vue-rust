const log = { steps: 'module' }
trace('module-init', log.steps, typeof run)
export function run() {
  trace('run-called', true)
  return 'ignored'
}
