function config({ host = 'localhost', port = 80 } = {}) {
  return `${host}:${port}`
}
trace('config', config(), config({ port: 8080 }), config({ host: 'h', port: undefined }))
