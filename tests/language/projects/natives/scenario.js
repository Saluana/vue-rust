// Native module imports: functions and constants answered by the runtime (natives.rs / natives-hooks.mjs).
import { identity, pair, box, answer, label, on } from '@vue-native/platform'
export function run() {
  trace('id', identity('x'))
  trace('pair', pair(1, 'two'))
  trace('box', box(true).value)
  trace('answer', answer + 1)
  trace('label', label)
  trace('on', on)
}
