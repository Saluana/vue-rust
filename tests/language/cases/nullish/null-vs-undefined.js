// @tier 1
// @covers L06
// A missing key reads as undefined; an explicit null is a different value. typeof and strict equality tell them apart.
const o = { present: null }
trace('values', o.present, o.missing, o.present === undefined, o.missing === null)
trace('types', typeof o.present, typeof o.missing, typeof null)
trace('loose', o.present == undefined, o.missing == null)
