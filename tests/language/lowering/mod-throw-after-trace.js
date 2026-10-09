const state = { step: 0 }
trace('state', state)
state.step = 1
trace('step', state.step)
throw { reason: 'stop', state }
