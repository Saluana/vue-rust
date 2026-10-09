// A named import from vue (a supported export of the profile) binds through the native module (WP-C1c).
import { ref } from 'vue'
export function run() {
  trace('ref', ref(3).value)
}
