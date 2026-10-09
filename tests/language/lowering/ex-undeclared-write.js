try {
  notDeclared = 3
} catch (e) {
  trace('write-error', e.name)
}
trace('no-global', typeof notDeclared)
