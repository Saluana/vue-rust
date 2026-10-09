// Relative specifiers with parent steps across directories.
import { deep } from './lib/inner/a.js'
export function run() {
  trace('deep', deep())
}
