import './cyc-a.js'
trace('cyc-b-eval')
export function getB() {
  return 'B'
}
export const pending = import('./cyc-a.js').then((ns) => {
  trace('settled', ns.name, ns.getA())
  return ns
})
