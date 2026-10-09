const v = 'x'
trace('nested-template', `[${`<${v}>`}]`, `${'a' + 'b'}`, `\${escaped}`)
