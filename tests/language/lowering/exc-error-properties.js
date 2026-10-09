const e = new Error('msg', { cause: 'why' })
trace('error', e.name, e.message, e.cause, e instanceof Error)
