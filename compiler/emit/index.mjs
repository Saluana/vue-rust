// Module emission (WP-C1b): HIR module -> Rust source. The file holds the prelude, the string pool, the ModuleDef static
// (`MODULE`, `def()`), the three phases (declare, bind, evaluate) and one Rust function per JavaScript function.
import { Emitter } from './buf.mjs';
import { PRELUDE } from './prelude.mjs';
import { emitExpr } from './expr.mjs';
import { emitStatements, emitHoist } from './stmt.mjs';
import { declareScope, bindPattern, initVar } from './vars.mjs';
import { localName, rstr, fnName } from './names.mjs';

// Runs body() with a fresh line buffer and returns the function text `header { ... }`.
function withBody(em, header, body) {
  const saved = { lines: em.lines, depth: em.depth, ctl: em.ctl, fnId: em.fnId, rec: em.rec };
  em.lines = [];
  em.depth = 1;
  em.ctl = [];
  try {
    body();
    return [`${header} {`, ...em.lines, '}'].join('\n');
  } finally {
    Object.assign(em, { lines: saved.lines, depth: saved.depth, ctl: saved.ctl, fnId: saved.fnId, rec: saved.rec });
  }
}

// One JavaScript function: captured cells first, then the self name, `this`, the scope bindings, parameters, hoisted
// functions and the body. Parameters: default values apply on undefined, rest collects the remaining arguments.
function emitFunction(em, rec) {
  return withBody(em, `fn ${fnName(rec.id, rec.name)}(rt: &Rt, call: &Call<'_>) -> Res<Value>`, () => {
    em.fnId = rec.id;
    em.rec = rec;
    em.line('rt.tick()?;');
    if (rec.caps.length > 0) {
      em.line('let kc = rt.captures(call.callee);');
      rec.caps.forEach((_, i) => em.line(`let k${i}: ObjRef = kc[${i}];`));
    }
    if (rec.selfB !== null) {
      if (rec.selfSt === 'cell') em.line(`let mut c${rec.selfB} = rt.new_cell(Value::Object(call.callee));`);
      else em.line(`let mut ${localName(rec.selfB, rec.selfName)}: Value = Value::Object(call.callee);`);
    }
    if (rec.thisCell) em.line(`let mut th${rec.id} = rt.new_cell(call.this.clone());`);
    declareScope(em, rec.scope.decls);
    rec.params.forEach((p, i) => {
      if (p.rest) {
        const v = em.tmp();
        em.line(`let ${v}: Value = vn_rest(rt, call.args, ${i})?;`);
        bindPattern(em, p.t, v);
        return;
      }
      const av = em.tmp();
      em.line(`let mut ${av}: Value = call.arg(${i});`);
      if (p.d) {
        em.line(`if matches!(${av}, Value::Undefined) {`);
        em.depth += 1;
        const d = emitExpr(em, p.d);
        em.line(`${av} = ${d}.clone();`);
        em.depth -= 1;
        em.line('}');
      }
      bindPattern(em, p.t, av);
    });
    emitHoist(em, rec.scope.hoist);
    emitStatements(em, rec.body);
    em.line('Ok(Value::Undefined)');
  });
}

// Phase bodies. The module cells and import cells are fetched again in bind and evaluate (they are not kept between
// phases), so every function reads the same ObjRefs.
function fetchCells(em, hir) {
  hir.slots.forEach((s, i) => em.line(`let mut m${s.id} = rt.module_local(md, ${i});`));
  for (const im of hir.imports) {
    em.line(`let i${im.id} = rt.module_import_cell(md, ${rstr(im.request)}, ${rstr(im.imported)})?;`);
  }
}

function emitDeclare(em, hir) {
  return withBody(em, 'fn vn_declare(rt: &Rt, md: ObjRef) -> Res<()>', () => {
    for (const s of hir.slots) {
      const init = s.init === 'ns'
        ? `Value::Object(rt.module_namespace(rt.module_request(md, ${rstr(s.request)})))`
        : s.init === 'undef' ? 'Value::Undefined' : 'Value::Uninit';
      em.line(`let mut m${s.id} = rt.new_cell(${init});`);
    }
    em.line(`rt.module_set_locals(md, vec![${hir.slots.map((s) => `m${s.id}`).join(', ')}]);`);
    for (const e of hir.exportsLocal) em.line(`rt.module_define_export(md, ${rstr(e.name)}, m${e.id});`);
    for (const r of hir.exportsIndirect) {
      em.line(`rt.module_add_reexport(md, ${rstr(r.name)}, ${rstr(r.request)}, ${rstr(r.imported)});`);
    }
    for (const req of hir.exportsStar) em.line(`rt.module_add_star_reexport(md, ${rstr(req)});`);
    em.line('Ok(())');
  });
}

function emitBind(em, hir) {
  return withBody(em, 'fn vn_bind(rt: &Rt, md: ObjRef) -> Res<()>', () => {
    fetchCells(em, hir);
    for (const h of hir.hoist) {
      const f = emitExpr(em, h.fn);
      initVar(em, h.target.r, h.target.name, f);
    }
    for (const e of hir.exportsNs) {
      em.line(`rt.module_define_export(md, ${rstr(e.name)}, rt.new_cell(Value::Object(rt.module_namespace(rt.module_request(md, ${rstr(e.request)})))));`);
    }
    em.line('Ok(())');
  });
}

function emitEvaluate(em, hir) {
  return withBody(em, 'fn vn_evaluate(rt: &Rt, md: ObjRef) -> Res<()>', () => {
    fetchCells(em, hir);
    emitStatements(em, hir.body);
    em.line('Ok(())');
  });
}

// Rust source of one module. Throws on an internal inconsistency (never silently skipped).
export function emitModule(hir) {
  if (hir.hirVersion !== 1) throw new Error(`emitModule: unsupported HIR version ${hir.hirVersion}`);
  const em = new Emitter(hir);
  em.expr = (n) => emitExpr(em, n);
  const phases = [emitDeclare(em, hir), emitBind(em, hir), emitEvaluate(em, hir)];
  const functions = hir.fns.filter(Boolean).map((rec) => emitFunction(em, rec));
  const requests = `&[${hir.requests.map(rstr).join(', ')}]`;
  const head = [
    `// Generated by the vue-native compiler (WP-C1b) from ${hir.specifier}. Do not edit.`,
    PRELUDE,
    ...em.strings,
    `pub static MODULE: ModuleDef = ModuleDef { specifier: ${rstr(hir.specifier)}, requests: ${requests}, declare: vn_declare, bind: vn_bind, evaluate: vn_evaluate };`,
    'pub fn def() -> &\'static ModuleDef {\n    &MODULE\n}',
  ];
  return [...head, ...phases, ...functions].join('\n\n') + '\n';
}
