export let counter = 0
counter += 1
trace('exported-let', counter)
export function run() {
  counter += 10
  trace('run-sees', counter)
}
