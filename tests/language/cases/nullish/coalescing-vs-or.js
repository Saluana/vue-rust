// @tier 1
// @covers L06
// ?? replaces only null and undefined; || also replaces 0, '', NaN and false.
const values = [0, '', null, undefined, NaN, false, 'x']
trace('nullish', values.map((v) => v ?? 'd'))
trace('or', values.map((v) => v || 'd'))
