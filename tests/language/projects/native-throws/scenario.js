// A native function that throws: the throw reaches the caller's try/catch, and finally still runs.
import { throws } from '@vue-native/platform'
export function run() {
  try {
    throws('boom')
  } catch (e) {
    trace('caught', e)
  } finally {
    trace('finally', true)
  }
  trace('end', 1)
}
