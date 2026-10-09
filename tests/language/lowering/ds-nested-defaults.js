const { n: { m = 'deep' } = {}, o = { p: 'q' } } = { n: undefined }
trace('nested-defaults', m, o.p)
