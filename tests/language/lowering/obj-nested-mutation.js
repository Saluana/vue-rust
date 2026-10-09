const cfg = { db: { host: 'a', opts: { ssl: false } } }
const alias = cfg.db
alias.opts.ssl = true
cfg.db.host += '!'
trace('nested', cfg, alias === cfg.db)
delete cfg.db.opts
trace('deleted', cfg.db)
