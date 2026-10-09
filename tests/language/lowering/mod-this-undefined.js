trace('module-this', this === undefined)
const arrow = () => this
trace('arrow-this', arrow() === undefined)
function plain() {
  return this
}
trace('plain', plain() === undefined)
