let effects = ''
const t = (x) => {
  effects += x
  return x
}
const r1 = t(0) && t('skip')
const r2 = t(1) || t('skip')
const r3 = t(null) ?? t('used')
const r4 = t(false) ? t('a') : t('b')
trace('short-circuit', r1, r2, r3, r4, effects)
