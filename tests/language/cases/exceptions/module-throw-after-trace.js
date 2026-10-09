// @tier 2
// @covers L13
// Entries traced before a module-level throw are kept; the thrown object is encoded as a fresh expansion at throw time.
const state = { step: 0 }
trace('state', state)
state.step = 1
trace('step', state.step)
throw { reason: 'stop', state }
