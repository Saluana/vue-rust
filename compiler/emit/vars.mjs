// Variables, scopes and destructuring in the emitted Rust (WP-C1b). A reference (HIR `r`) is a Rust local `v<id>_<name>`
// (L), a cell `c<id>` (C), a captured cell `k<i>` (K), a module cell `m<id>` (M) or an import cell `i<id>` (I). Cells are
// read with cell_get / cell_get_checked; locals in the TDZ are read with vn_lget (Uninit is the TDZ marker).
// Expressions are emitted through em.expr (set by the module emitter), so this file does not import expr.mjs.
import { localName, cellName, rstr } from './names.mjs';

const decl = (t, expr) => `let ${t}: Value = ${expr};`;

export function cellExpr(r) {
  if (r.k === 'C') return typeof r.id === 'string' ? r.id : cellName(r.id);
  if (r.k === 'K') return `k${r.i}`;
  if (r.k === 'M') return `m${r.id}`;
  if (r.k === 'I') return `i${r.id}`;
  throw new Error(`no cell for reference ${r.k}`);
}

// Reads a binding into a fresh temporary (the read happens here, before any later side effect).
export function readVar(em, r, name, tdz) {
  const t = em.tmp();
  if (r.k === 'L') {
    const v = localName(r.id, name);
    em.line(decl(t, tdz ? `vn_lget(rt, &${v}, ${rstr(name)})?` : `${v}.clone()`));
  } else if (tdz) {
    em.line(decl(t, `rt.cell_get_checked(${cellExpr(r)}, ${rstr(name)})?`));
  } else {
    em.line(decl(t, `rt.cell_get(${cellExpr(r)})`));
  }
  return t;
}

// Initialises a binding (a declaration or a hoisted function): no TDZ or const check.
export function initVar(em, r, name, value) {
  if (r.k === 'L') em.line(`${localName(r.id, name)} = ${value}.clone();`);
  else em.line(`rt.cell_init(${cellExpr(r)}, ${value}.clone());`);
}

// Assignment to a binding (PutValue, strict). A const, an import or a function-expression name throws a TypeError after
// the TDZ check, so an uninitialised binding reports ReferenceError first.
export function assignVar(em, target, value) {
  const { r, name, tdz, ro } = target;
  if (ro) {
    if (tdz) em.line(`${readCheck(r, name)};`);
    em.line('return Err(vn_const_error(rt));');
    return;
  }
  if (r.k === 'L') {
    const v = localName(r.id, name);
    if (tdz) em.line(`vn_lset(rt, &mut ${v}, ${value}.clone(), ${rstr(name)})?;`);
    else em.line(`${v} = ${value}.clone();`);
  } else if (tdz) {
    em.line(`rt.cell_set_checked(${cellExpr(r)}, ${rstr(name)}, ${value}.clone())?;`);
  } else {
    em.line(`rt.cell_set(${cellExpr(r)}, ${value}.clone());`);
  }
}

function readCheck(r, name) {
  if (r.k === 'L') return `vn_lget(rt, &${localName(r.id, name)}, ${rstr(name)})?`;
  return `rt.cell_get_checked(${cellExpr(r)}, ${rstr(name)})?`;
}

// Declares the bindings of a scope. Cells for captured bindings, Rust locals otherwise. Vars start undefined, every
// other binding starts uninitialised (the TDZ); parameters are initialised by the prologue, in order.
export function declareScope(em, decls) {
  for (const d of decls) {
    const init = d.kind === 'var' ? 'Value::Undefined' : 'Value::Uninit';
    if (d.st === 'cell') em.line(`let mut ${cellName(d.id)} = rt.new_cell(${init});`);
    else em.line(`let mut ${localName(d.id, d.name)}: Value = ${init};`);
  }
}

// The member target of an assignment (or a destructuring leaf): base and key are evaluated before the value is read.
export function prepareMember(em, t) {
  const o = em.tmp();
  em.line(decl(o, `${em.expr(t.o)}.clone()`));
  if (t.key.k === 's') return { o, kv: null, name: t.key.s };
  const kv = em.tmp();
  em.line(decl(kv, `${em.expr(t.key.e)}.clone()`));
  return { o, kv, name: null };
}

export function putPrepared(em, prep, value) {
  if (prep.name !== null) {
    em.line(`rt.set_v_strict(&${prep.o}, &PropKey::from_str(${rstr(prep.name)}), ${value}.clone())?;`);
  } else {
    em.line(`vn_put(rt, &${prep.o}, &${prep.kv}, ${value}.clone())?;`);
  }
}

// Binds the value in the temporary `value` to a pattern leaf or nested pattern.
// `prep` is a prepared member target for a leaf (already evaluated by the caller, in spec order).
export function bindPattern(em, pat, value, prep = null) {
  switch (pat.k) {
    case 'pinit':
      initVar(em, pat.r, pat.name, value);
      return;
    case 'tvar':
      assignVar(em, pat, value);
      return;
    case 'tglobal':
      em.line(`vn_gset(rt, ${rstr(pat.name)}, ${value}.clone())?;`);
      return;
    case 'tmem':
      putPrepared(em, prep ?? prepareMember(em, pat), value);
      return;
    case 'pobj':
      bindObject(em, pat, value);
      return;
    case 'parr':
      bindArray(em, pat, value);
      return;
    default:
      throw new Error(`bindPattern: unknown pattern ${pat.k}`);
  }
}

// An element `{ t, d }`: the default applies when the value is undefined; `v` is a temporary holding the value.
function bindElement(em, el, v, prep) {
  let cur = v;
  if (el.d) {
    cur = em.tmp();
    em.line(`let mut ${cur}: Value = ${v}.clone();`);
    em.line(`if matches!(${cur}, Value::Undefined) {`);
    em.depth += 1;
    const d = em.expr(el.d);
    em.line(`${cur} = ${d}.clone();`);
    em.depth -= 1;
    em.line('}');
  }
  bindPattern(em, el.t, cur, prep);
}

// The leaf of an element that is a member target, prepared before the value is fetched.
function leafPrep(em, el) {
  return el && el.t.k === 'tmem' ? prepareMember(em, el.t) : null;
}

// Object pattern: RequireObjectCoercible, then per property: key, target reference, GetV, default, binding. The rest
// object copies the own enumerable properties that were not named.
function bindObject(em, pat, value) {
  em.line(`vn_require_obj(rt, &${value})?;`);
  const hasRest = pat.rest !== null;
  const ex = em.tmp();
  if (hasRest) em.line(`let mut ${ex}: Vec<PropKey> = Vec::new();`);
  for (const p of pat.props) {
    const kk = em.tmp();
    if (p.key.k === 's') {
      em.line(`let ${kk}: PropKey = PropKey::from_str(${rstr(p.key.s)});`);
    } else {
      const kv = em.tmp();
      em.line(decl(kv, `${em.expr(p.key.e)}.clone()`));
      em.line(`let ${kk}: PropKey = vn_key(rt, &${kv})?;`);
    }
    if (hasRest) em.line(`${ex}.push(${kk}.clone());`);
    const prep = leafPrep(em, p.t ? { t: p.t } : null);
    const v = em.tmp();
    em.line(decl(v, `rt.get_v(&${value}, &${kk})?`));
    bindElement(em, { t: p.t, d: p.d }, v, prep);
  }
  if (hasRest) {
    const prep = leafPrep(em, { t: pat.rest });
    const ro = em.tmp();
    em.line(`let ${ro} = rt.new_object();`);
    em.line(`vn_copy_props(rt, ${ro}, &${value}, &${ex})?;`);
    const rv = em.tmp();
    em.line(decl(rv, `Value::Object(${ro})`));
    bindPattern(em, pat.rest, rv, prep);
  }
}

// Array pattern: the iterator is opened once. Element targets run inside a closure so that an abrupt completion closes
// the iterator (throw completion: the original error wins). A normal finish closes it unless it is done.
function bindArray(em, pat, value) {
  const it = em.uniq('it');
  const res = em.uniq('r');
  em.line(`let mut ${it} = get_iterator(rt, &${value})?;`);
  em.line(`let ${res}: Res<()> = (|| -> Res<()> {`);
  em.depth += 1;
  for (const el of pat.elems) {
    if (el === null) {
      em.line(`iter_step(rt, &mut ${it})?;`);
      continue;
    }
    const prep = leafPrep(em, el);
    const v = em.tmp();
    em.line(`let ${v}: Value = match iter_step(rt, &mut ${it})? { Some(x) => x, None => Value::Undefined };`);
    bindElement(em, el, v, prep);
  }
  if (pat.rest !== null) {
    const prep = leafPrep(em, { t: pat.rest });
    const acc = em.tmp();
    em.line(`let mut ${acc}: Vec<Value> = Vec::new();`);
    em.line(`while let Some(x) = iter_step(rt, &mut ${it})? { ${acc}.push(x); }`);
    const av = em.tmp();
    em.line(decl(av, `vn_array(rt, ${acc})?`));
    bindPattern(em, pat.rest, av, prep);
  }
  em.line('Ok(())');
  em.depth -= 1;
  em.line('})();');
  em.line(`match ${res} {`);
  em.line(`    Ok(()) => { if !${it}.done { vn_iter_close_normal(rt, &${it})?; } }`);
  em.line(`    Err(e) => { if !${it}.done { vn_iter_close_throw(rt, &${it}); } return Err(e); }`);
  em.line('}');
}
