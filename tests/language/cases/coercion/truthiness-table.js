// @tier 1
// @covers L07
// Falsy primitives are 0, -0, NaN, '', null, undefined and false; arrays, objects and '0' are truthy.
const samples = [0, -0, NaN, '', '0', null, undefined, false, [], {}, 'false', 1]
trace('samples', samples)
trace('truthy', samples.map((v) => !!v))
