trace('nullish', 0 ?? 'd', '' ?? 'd', null ?? 'd', undefined ?? 'd', false ?? 'd', NaN ?? 'd')
const o = { v: 0 }
trace('nullish-member', o.v ?? 5, o.missing ?? 6)
