// Negative control of the Test262 runner (WP 3.1), not part of the selection. It is a copy of
// test/built-ins/Math/abs/S15.8.2.1_A1.js, which passes, with the expected value NaN changed to 0. The runner must report
// it as a fail: node tooling/harness/run-test262.mjs --dir tests/language/test262/controls --no-ledger exits 1.
/*---
info: If x is NaN, Math.abs(x) is NaN
es5id: 15.8.2.1_A1
description: Negative control. The expected value is wrong (0 instead of NaN), so the test must fail.
---*/

assert.sameValue(Math.abs(NaN), 0);
