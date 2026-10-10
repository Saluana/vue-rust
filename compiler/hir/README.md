# HIR node catalogue (hirVersion 2)

Module: `{ hirVersion, kind:'Module', specifier, origin, lang, requests, slots, imports, exportsLocal,
exportsIndirect, exportsStar, exportsNs, hoist, body, fns }`. `slots` are the module cells (index = order in `vn_declare`).
Expressions, targets, patterns and statements have `k` and `pos: [start, end]` (UTF-16 offsets); references, keys and
for-in/of heads have `k` only.

References `r` (where the value lives, seen from the current function): `L` Rust local, `C` owned cell (`id` may be
the string `th<fn>` for `this`), `K` captured cell (`i` = index in the function's captures), `M` module cell, `I` import cell.

Expressions: `undef null bool num(s: JS number text) str(u: UTF-16 units) var(r,name,tdz) gvar(name) thisval thiscell(r)
fn(f, caps) await(e) arr(elems: expr|spread|hole) obj(props: init|get|set|spread) mem(o,key) call(f,args) new(f,args) chain(base,
links: m|c with opt) un(op,e) typeof(e) void(e) del(o,key) delv(e) bin(op,l,r) log(op,l,r) cond(t,c,a) seq(es)
asgn(op,t,v) asgnp(pat,v) logasgn(op,t,v) upd(prefix,inc,t) tmpl(parts) dimport(request)`. `dimport` is `import()` with a
literal specifier (a string or a no-substitution template); `request` is the target's id from the module resolver, the one
static requests use. The target is not a request of the module: the project graph loads it.
Targets: `tvar(r,name,tdz,ro)`, `tglobal(name)`, `tmem(o,key)`.
Patterns: `pinit(r,name,tdz)` (declaration), targets as above, `pobj(props: {key,t,d}, rest)`, `parr(elems: {t,d}|null, rest)`.
Keys: `{k:'s', s}` static or `{k:'c', e}` computed.

Statements: `expr(e) decl(t,v) seqdecl(list) block(scope,body) if(t,c,a) while(lid,t,body) dowhile(lid,t,body)
for(lid,scope{decls,perIter},init,t,upd,body) forof(lid,scope,left{k:decl|target},right,body) forin(...)
switch(lid,disc,scope,cases{t,body}) try(block,handler{scope,param,body},fin) ret(e) throw(e) break(lid) continue(lid)
label(lid,body) empty`. A scope is `{decls:[{id,name,kind,st:'cell'|'local',tdz}], hoist:[{target,fn}]}`.

Functions (`fns[id]`): `{id,name,nameFromKey,length,kind,arrow,s,e,caps,selfB,selfName,selfSt,thisCell,async,scope,params:
[{t,d,rest}],paramCopies:[{to,from,name}],body}`. `async` functions (WP-C1c) contain `await` nodes; `for await` is rejected.

The catalogue is data in `schema.mjs` (`HIR_NODES`, `HIR_RECORDS`). `HIR_SCHEMA_HASH` is the sha256 of its canonical JSON.
`tests/compiler/lower/hir-schema.test.mjs` walks the HIR of the lowering fixtures and fails on any node kind, field or record
shape that the catalogue does not list, and on any catalogue entry no fixture produces. A change of shape must bump
`HIR_VERSION` and add the new hash to that test's `COMMITTED` table.

hirVersion 2 (task 3.7): `await` and `throw` nodes and function records carry `site`, an index into the module's `sites`
table (`{kind,name,file,line,column}`, the source positions of uncaught-throw frames).
