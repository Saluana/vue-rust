const o = { x: 1, y: 2 }
trace('void', void 0, void 'x', void o.x)
trace('delete', delete o.x, o.x, 'x' in o, delete o.missing, 'y' in o)
