// @tier 1
// @covers L08
// Number-to-string printing uses the shortest round-trip form, not decimal intuition.
trace('sum', 0.1 + 0.2, 0.1 + 0.2 === 0.3)
trace('big', 1e21, 1e20, 123456789012345680000)
trace('small', 1e-7, 0.000001, 1.5e-10)
trace('strings', String(1e21), String(-0), String(0.1 * 3), `${2 ** 53}`)
trace('neg-zero', -0, 0 * -1, 1 / -0, -0 === 0)
