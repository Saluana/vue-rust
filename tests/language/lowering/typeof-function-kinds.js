const kinds = [typeof (() => 1), typeof function () {}, typeof (function named() {}), typeof class0]
function class0() {}
trace('kinds', kinds)
