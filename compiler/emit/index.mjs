// Module emission (WP-C1b): HIR module -> Rust source. The file holds the prelude, the string pool, the ModuleDef static
// (`MODULE`, `def()`), the three phases (declare, bind, evaluate) and one Rust function per JavaScript function.
import { Emitter } from './buf.mjs';
import { PRELUDE, ASYNC_PRELUDE } from './prelude.mjs';
import { emitExpr } from './expr.mjs';
import { emitStatements, emitHoist } from './stmt.mjs';
import { declareScope, bindPattern, initVar, readVar } from './vars.mjs';
import { localName, rstr, fnName, owned } from './names.mjs';
import { HIR_VERSION } from '../hir/context.mjs';

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
  if (rec.async) return emitAsyncFunction(em, rec);
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
        em.line(`${av} = ${owned(d)};`);
        em.depth -= 1;
        em.line('}');
      }
      bindPattern(em, p.t, av);
    });
    for (const copy of rec.paramCopies ?? []) {
      const value = readVar(em, copy.from, copy.name, false);
      initVar(em, copy.to, copy.name, value);
    }
    emitHoist(em, rec.scope.hoist);
    emitStatements(em, rec.body);
    em.line('Ok(Value::Undefined)');
  });
}

// An async function (WP-C1c) is two Rust items. The entry `fN(rt, call)` creates the heap frame, stores this, the callee,
// the captured cells and the raw arguments in their fixed slots (0, 1, 2.., see buf.mjs), and starts the body with
// async_start, which returns the result promise. The body `fN_body(rt_, fr)` is an async fn: it binds the parameters
// from the frame and runs the statements. No Value is held in a Rust local across an await (see buf.mjs).
function emitAsyncFunction(em, rec) {
  const caps = rec.caps.length;
  const wrapper = fnName(rec.id, rec.name);
  const bodyName = `${wrapper}_body`;
  em.async = true;
  em.startFrame(caps, rec.params.length);
  let body;
  try {
    body = withBody(em, `async fn ${bodyName}(rt_: Rt, fr: ObjRef) -> Res<Value>`, () => {
      em.fnId = rec.id;
      em.rec = rec;
      em.line('let rt: &Rt = &rt_;');
      em.line('rt.tick()?;');
      if (rec.selfB !== null) {
        const slot = em.slotFor(`b:${rec.selfB}`);
        if (rec.selfSt === 'cell') em.line(`frame_set(rt, fr, ${slot}, Value::Object(rt.new_cell(frame_get(rt, fr, 1))));`);
        else em.line(`frame_set(rt, fr, ${slot}, frame_get(rt, fr, 1));`);
      }
      if (rec.thisCell) em.line(`frame_set(rt, fr, ${em.slotFor(`th${rec.id}`)}, Value::Object(rt.new_cell(frame_get(rt, fr, 0))));`);
      declareScope(em, rec.scope.decls);
      rec.params.forEach((p, i) => {
        const from = `frame_get(rt, fr, ${2 + caps + i})`;
        if (p.d) {
          const av = em.tmp();
          em.defMut(av, from);
          em.line(`if matches!(${av}, Value::Undefined) {`);
          em.depth += 1;
          const d = emitExpr(em, p.d);
          em.set(av, owned(d));
          em.depth -= 1;
          em.line('}');
          bindPattern(em, p.t, av);
          return;
        }
        bindPattern(em, p.t, from);
      });
      for (const copy of rec.paramCopies ?? []) {
        const value = readVar(em, copy.from, copy.name, false);
        initVar(em, copy.to, copy.name, value);
      }
      emitHoist(em, rec.scope.hoist);
      emitStatements(em, rec.body);
      em.line('Ok(Value::Undefined)');
    });
  } finally {
    em.async = false;
  }
  const frameSize = em.fs.next;
  em.fs = null;
  const entry = [
    `fn ${wrapper}(rt: &Rt, call: &Call<'_>) -> Res<Value> {`,
    `    let fr: ObjRef = new_frame(rt, ${frameSize});`,
    '    frame_set(rt, fr, 0, call.this.clone());',
    '    frame_set(rt, fr, 1, Value::Object(call.callee));',
  ];
  if (caps > 0) {
    entry.push('    let kc = rt.captures(call.callee);');
    rec.caps.forEach((_, i) => entry.push(`    frame_set(rt, fr, ${2 + i}, Value::Object(kc[${i}]));`));
  }
  rec.params.forEach((p, i) => {
    const slot = 2 + caps + i;
    if (p.rest) entry.push(`    frame_set(rt, fr, ${slot}, vn_rest(rt, call.args, ${i})?);`);
    else entry.push(`    frame_set(rt, fr, ${slot}, call.arg(${i}));`);
  });
  entry.push(`    Ok(async_start(rt, fr, Box::pin(${bodyName}(rt.clone(), fr))))`, '}');
  return [entry.join('\n'), body].join('\n\n');
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
  if (hir.hirVersion !== HIR_VERSION) throw new Error(`emitModule: unsupported HIR version ${hir.hirVersion}`);
  const em = new Emitter(hir);
  em.expr = (n) => emitExpr(em, n);
  const phases = [emitDeclare(em, hir), emitBind(em, hir), emitEvaluate(em, hir)];
  const functions = hir.fns.filter(Boolean).map((rec) => emitFunction(em, rec));
  const requests = `&[${hir.requests.map(rstr).join(', ')}]`;
  const usesAsync = hir.fns.some((f) => f && f.async);
  // The source sites of the module (hir.sites, compiler/hir/context.mjs siteOf): the author's place of each function entry,
  // throw and await. Read by Rt::throw_at and Rt::source_frames (vn-value error.rs).
  const siteKind = { fn: 'Function', throw: 'Throw', await: 'Await' };
  const sites = hir.sites.map((s) => `    SourceSite { kind: SiteKind::${siteKind[s.kind]}, name: ${rstr(s.name)}, file: ${rstr(s.file)}, line: ${s.line}, column: ${s.column} },`);
  const head = [
    `// Generated by the vue-native compiler (WP-C1b) from ${hir.specifier}. Do not edit.`,
    PRELUDE,
    ...(usesAsync ? [ASYNC_PRELUDE] : []),
    ...em.strings,
    sites.length === 0 ? 'pub static SITES: &[SourceSite] = &[];' : `pub static SITES: &[SourceSite] = &[\n${sites.join('\n')}\n];`,
    `pub static MODULE: ModuleDef = ModuleDef { specifier: ${rstr(hir.specifier)}, requests: ${requests}, declare: vn_declare, bind: vn_bind, evaluate: vn_evaluate };`,
    'pub fn def() -> &\'static ModuleDef {\n    &MODULE\n}',
  ];
  return [...head, ...phases, ...functions].join('\n\n') + '\n';
}
