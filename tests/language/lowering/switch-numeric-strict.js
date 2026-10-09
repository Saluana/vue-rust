const pick = (v) => {
  switch (v) {
    case 1:
      return 'int'
    case '1':
      return 'str'
    case true:
      return 'bool'
    default:
      return 'other'
  }
}
trace('strict-switch', pick(1), pick('1'), pick(true), pick(1.0))
