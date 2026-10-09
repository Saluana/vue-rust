// @tier 2
// @covers L08
// toFixed rounds the exact binary value; exact decimal ties round toward the larger n.
trace('fixed', (1.005).toFixed(2), (1.45).toFixed(1), (2.5).toFixed(0), (-2.5).toFixed(0))
trace('fixed-more', (0.5).toFixed(0), (1.5).toFixed(0), (1e21).toFixed(2), (0).toFixed(3))
trace('fixed-neg-zero', (-0.0001).toFixed(2), (-0).toFixed(1))
