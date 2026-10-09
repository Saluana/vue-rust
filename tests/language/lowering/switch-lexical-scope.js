function run(v) {
  switch (v) {
    case 'a':
      let shared = 'from-a'
      return shared
    case 'b':
      shared = 'from-b'
      return shared
    default:
      return 'none'
  }
}
trace('switch-lex', run('a'), run('z'))
