let log = ''
const o = {}
function key() { log += 'key;'; return 'k' }
function val() { log += 'val;'; return 7 }
o[key()] = val()
trace('order-assign', log, o.k)
