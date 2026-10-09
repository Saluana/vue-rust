// Expression emission (WP-C1b, extended by WP-C1c). emitExpr(em, node) writes the statements that evaluate the node into
// em's buffer and returns a Rust expression of type Value: a temporary (for anything with effects or reads) or a constant.
// Operands are evaluated into temporaries in source order, so the Rust order is the JavaScript order. In an async function
// the temporaries are frame slots (buf.mjs); argument and element lists are accumulators (vn_acc_* in the prelude), and
// await is `await_value(rt, v).await?` stored in a slot.
import { readVar, cellExpr, assignVar, prepareMember, putPrepared, bindPattern } from './vars.mjs';
import { rstr, f64Lit, fnName, owned, vecNew } from './names.mjs';

const BIN_FN = {
  '+': 'add', '-': 'sub', '*': 'mul', '/': 'div', '%': 'rem', '**': 'pow', '<<': 'shl', '>>': 'shr', '>>>': 'ushr',
  '&': 'bit_and', '|': 'bit_or', '^': 'bit_xor', '<': 'lt', '>': 'gt', '<=': 'le', '>=': 'ge',
};
const FN_KIND = { normal: 'Normal', arrow: 'Arrow', method: 'Method', getter: 'Getter', setter: 'Setter' };

function bindTmp(em, expr) {
  const t = em.tmp();
  em.def(t, expr);
  return t;
}

// Applies a compound operator: `op` is a binary operator text (without the =).
function binaryOp(em, op, l, r) {
  if (op === '===') return `Value::Bool(strict_eq(&${l}, &${r}))`;
  if (op === '!==') return `Value::Bool(!strict_eq(&${l}, &${r}))`;
  if (op === '==') return `rt.loose_eq(&${l}, &${r})?`;
  if (op === '!=') return `Value::Bool(!vn_truth(&rt.loose_eq(&${l}, &${r})?))`;
  if (op === 'in') return `Value::Bool(rt.in_operator(&${l}, &${r})?)`;
  if (op === 'instanceof') return `Value::Bool(rt.instance_of(&${l}, &${r})?)`;
  const fn = BIN_FN[op];
  if (!fn) throw new Error(`no operator ${op}`);
  return `rt.${fn}(&${l}, &${r})?`;
}

// Arguments as a Vec<Value> expression (spread expands the iterable in order). Synchronously the Vec is a Rust local; in
// an async function the values go to an accumulator held in a frame slot and the Vec is taken when the call happens.
function emitArgs(em, args) {
  if (em.async) {
    const acc = em.tmp();
    em.def(acc, 'vn_acc_new(rt)');
    for (const a of args) {
      if (a.k === 'spread') {
        const x = emitExpr(em, a.e);
        em.line(`vn_acc_spread(rt, &${acc}, &${x})?;`);
      } else {
        const x = emitExpr(em, a);
        em.line(`vn_acc_push(rt, &${acc}, ${owned(x)})?;`);
      }
    }
    return `vn_acc_take(rt, &${acc})?`;
  }
  const va = em.uniq('a');
  const spread = args.some((a) => a.k === 'spread');
  em.line(`let mut ${va}: Vec<Value> = ${vecNew(spread ? null : args.length)};`);
  for (const a of args) {
    if (a.k === 'spread') {
      const x = emitExpr(em, a.e);
      em.line(`vn_spread(rt, &mut ${va}, &${x})?;`);
    } else {
      const x = emitExpr(em, a);
      em.line(`${va}.push(${owned(x)});`);
    }
  }
  return va;
}

// Reads a member value: static names need no conversion; computed keys go through vn_get (nullish base first).
function getMember(em, o, key) {
  if (key.k === 's') return bindTmp(em, `rt.get_v(&${o}, &PropKey::lit(${rstr(key.s)}))?`);
  const kv = emitExpr(em, key.e);
  return bindTmp(em, `vn_get(rt, &${o}, &${kv})?`);
}

function fnCreate(em, n) {
  const rec = em.hir.fns[n.f];
  if (!rec) throw new Error(`no function record ${n.f}`);
  const caps = n.caps.map((r) => {
    if (r.k === 'L') throw new Error('a captured binding is a Rust local (analysis mismatch)');
    return cellExpr(em, r);
  });
  const name = rec.name ?? '';
  const kind = rec.async ? 'Async' : FN_KIND[rec.kind];
  return `Value::Object(rt.new_function(${fnName(n.f, rec.name)}, FnInfo::new(${rstr(name)}, ${rec.length}, FnKind::${kind}), &[${caps.join(', ')}]))`;
}

// Lowers an optional chain. Result: { v, t } temporaries; t is the `this` of the final member link (for a callee).
function emitChain(em, n) {
  const label = em.uniq('oc');
  const v = em.tmp();
  const th = em.tmp();
  em.defMut(v, 'Value::Undefined');
  em.defMut(th, 'Value::Undefined');
  const base = emitExpr(em, n.base);
  const cur = em.tmp();
  em.defMut(cur, owned(base));
  em.line(`'${label}: {`);
  em.depth += 1;
  let prevMember = false;
  let thisv = null;
  for (const link of n.links) {
    if (link.k === 'm') {
      if (link.opt) em.line(`if ${cur}.is_nullish() { break '${label}; }`);
      const obj = em.tmp();
      em.def(obj, `${cur}.clone()`);
      if (link.key.k === 's') {
        em.set(cur, `rt.get_v(&${obj}, &PropKey::lit(${rstr(link.key.s)}))?`);
      } else {
        const kv = emitExpr(em, link.key.e);
        em.set(cur, `vn_get(rt, &${obj}, &${kv})?`);
      }
      thisv = obj;
      prevMember = true;
    } else {
      if (link.opt) em.line(`if ${cur}.is_nullish() { break '${label}; }`);
      const args = emitArgs(em, link.args);
      const callThis = prevMember && thisv !== null ? thisv : null;
      em.set(cur, `rt.call(&${cur}, &${callThis ? `${callThis}.clone()` : 'Value::Undefined'}, &${args})?`);
      thisv = null;
      prevMember = false;
    }
  }
  em.set(v, `${cur}.clone()`);
  if (thisv !== null && prevMember) em.set(th, `${thisv}.clone()`);
  em.depth -= 1;
  em.line('}');
  return { v, t: th };
}

// Assignment operators on a member target: the key is converted by vn_get and again by vn_put (V8 does the same).
function memberRead(em, prep) {
  if (prep.name !== null) return bindTmp(em, `rt.get_v(&${prep.o}, &PropKey::lit(${rstr(prep.name)}))?`);
  return bindTmp(em, `vn_get(rt, &${prep.o}, &${prep.kv})?`);
}

function memberWrite(em, prep, value) {
  putPrepared(em, prep, value);
}

// Target-directed read of an identifier target (a tvar read in TDZ-aware form, or a global).
function targetRead(em, t) {
  if (t.k === 'tvar') return readVar(em, t.r, t.name, t.tdz);
  return bindTmp(em, `vn_gget(rt, ${rstr(t.name)})?`);
}

function targetWrite(em, t, value) {
  if (t.k === 'tvar') assignVar(em, t, value);
  else em.line(`vn_gset(rt, ${rstr(t.name)}, ${owned(value)})?;`);
}

function emitAssign(em, n) {
  const t = n.t;
  if (n.op === '=') {
    if (t.k === 'tmem') {
      const prep = prepareMember(em, t);
      const v = emitExpr(em, n.v);
      memberWrite(em, prep, v);
      return v;
    }
    const v = emitExpr(em, n.v);
    targetWrite(em, t, v);
    return v;
  }
  if (t.k === 'tmem') {
    const prep = prepareMember(em, t);
    const old = memberRead(em, prep);
    const r = emitExpr(em, n.v);
    const nv = bindTmp(em, binaryOp(em, n.op, old, r));
    memberWrite(em, prep, nv);
    return nv;
  }
  const old = targetRead(em, t);
  const r = emitExpr(em, n.v);
  const nv = bindTmp(em, binaryOp(em, n.op, old, r));
  targetWrite(em, t, nv);
  return nv;
}

// `a ||= b`, `a &&= b`, `a ??= b`: the right side runs only when the test asks for the write.
function emitLogAssign(em, n) {
  const t = n.t;
  const res = em.tmp();
  const cond = (cur) => {
    if (n.op === '&&') return `vn_truth(&${cur})`;
    if (n.op === '||') return `!vn_truth(&${cur})`;
    return `${cur}.is_nullish()`;
  };
  if (t.k === 'tmem') {
    const prep = prepareMember(em, t);
    const cur = memberRead(em, prep);
    em.defMut(res, `${cur}.clone()`);
    em.line(`if ${cond(cur)} {`);
    em.depth += 1;
    const r = emitExpr(em, n.v);
    memberWrite(em, prep, r);
    em.set(res, owned(r));
    em.depth -= 1;
    em.line('}');
    return res;
  }
  const cur = targetRead(em, t);
  em.defMut(res, `${cur}.clone()`);
  em.line(`if ${cond(cur)} {`);
  em.depth += 1;
  const r = emitExpr(em, n.v);
  targetWrite(em, t, r);
  em.set(res, owned(r));
  em.depth -= 1;
  em.line('}');
  return res;
}

// ++ and --: the old value is converted to a number once; prefix returns the new value, postfix the old one.
function emitUpdate(em, n) {
  const t = n.t;
  const step = n.inc ? 'inc' : 'dec';
  if (t.k === 'tmem') {
    const prep = prepareMember(em, t);
    const cur = memberRead(em, prep);
    const oldn = bindTmp(em, `Value::Number(rt.to_numeric(&${cur})?)`);
    const nv = bindTmp(em, `rt.${step}(&${oldn})?`);
    memberWrite(em, prep, nv);
    return n.prefix ? nv : oldn;
  }
  const cur = targetRead(em, t);
  const oldn = bindTmp(em, `Value::Number(rt.to_numeric(&${cur})?)`);
  const nv = bindTmp(em, `rt.${step}(&${oldn})?`);
  targetWrite(em, t, nv);
  return n.prefix ? nv : oldn;
}

// Lowers one HIR expression. Returns a Rust expression of type Value (a temporary or a constant).
export function emitExpr(em, n) {
  switch (n.k) {
    case 'undef':
      return 'Value::Undefined';
    case 'null':
      return 'Value::Null';
    case 'bool':
      return `Value::Bool(${n.v ? 'true' : 'false'})`;
    case 'num':
      return `Value::Number(${f64Lit(n.s)})`;
    case 'str':
      return `Value::String(JsStr::from_units(${em.strConst(n.u)}))`;
    case 'var':
      return readVar(em, n.r, n.name, n.tdz);
    case 'gvar':
      return bindTmp(em, `vn_gget(rt, ${rstr(n.name)})?`);
    case 'thisval':
      return bindTmp(em, em.async ? 'frame_get(rt, fr, 0)' : 'call.this.clone()');
    case 'thiscell':
      return bindTmp(em, `rt.cell_get(${cellExpr(em, n.r)})`);
    case 'fn':
      return bindTmp(em, fnCreate(em, n));
    case 'arr': {
      if (em.async) {
        const acc = em.tmp();
        em.def(acc, 'vn_acc_new(rt)');
        for (const el of n.elems) {
          if (el.k === 'hole') em.line(`vn_acc_push(rt, &${acc}, Value::Hole)?;`);
          else if (el.k === 'spread') {
            const x = emitExpr(em, el.e);
            em.line(`vn_acc_spread(rt, &${acc}, &${x})?;`);
          } else {
            const x = emitExpr(em, el);
            em.line(`vn_acc_push(rt, &${acc}, ${owned(x)})?;`);
          }
        }
        return bindTmp(em, `vn_array(rt, vn_acc_take(rt, &${acc})?)?`);
      }
      const va = em.uniq('e');
      const spread = n.elems.some((el) => el.k === 'spread');
      em.line(`let mut ${va}: Vec<Value> = ${vecNew(spread ? null : n.elems.length)};`);
      for (const el of n.elems) {
        if (el.k === 'hole') em.line(`${va}.push(Value::Hole);`);
        else if (el.k === 'spread') {
          const x = emitExpr(em, el.e);
          em.line(`vn_spread(rt, &mut ${va}, &${x})?;`);
        } else {
          const x = emitExpr(em, el);
          em.line(`${va}.push(${owned(x)});`);
        }
      }
      return bindTmp(em, `vn_array(rt, ${va})?`);
    }
    case 'obj':
      return emitObject(em, n);
    case 'mem': {
      const o = emitExpr(em, n.o);
      return getMember(em, o, n.key);
    }
    case 'call':
      return emitCall(em, n);
    case 'new': {
      const f = emitExpr(em, n.f);
      const args = emitArgs(em, n.args);
      return bindTmp(em, `rt.construct(&${f}, &${args}, &${owned(f)})?`);
    }
    case 'chain':
      return emitChain(em, n).v;
    case 'un': {
      const x = emitExpr(em, n.e);
      if (n.op === '-') return bindTmp(em, `rt.neg(&${x})?`);
      if (n.op === '+') return bindTmp(em, `rt.pos(&${x})?`);
      if (n.op === '!') return bindTmp(em, `Value::Bool(rt.not_(&${x}))`);
      return bindTmp(em, `rt.bit_not(&${x})?`);
    }
    case 'typeof': {
      if (n.e.k === 'gvar') {
        const v = bindTmp(em, `vn_gtypeof(rt, ${rstr(n.e.name)})?`);
        return bindTmp(em, `Value::str(rt.typeof_(&${v}))`);
      }
      const x = emitExpr(em, n.e);
      return bindTmp(em, `Value::str(rt.typeof_(&${x}))`);
    }
    case 'void': {
      emitExpr(em, n.e);
      return 'Value::Undefined';
    }
    case 'del': {
      const o = emitExpr(em, n.o);
      const key = n.key.k === 's'
        ? `Value::String(JsStr::from_str(${rstr(n.key.s)}))`
        : emitExpr(em, n.key.e);
      return bindTmp(em, `vn_delete(rt, &${o}, &${key})?`);
    }
    case 'delv': {
      emitExpr(em, n.e);
      return 'Value::Bool(true)';
    }
    case 'bin': {
      const l = emitExpr(em, n.l);
      const r = emitExpr(em, n.r);
      return bindTmp(em, binaryOp(em, n.op, l, r));
    }
    case 'log': {
      const res = em.tmp();
      const l = emitExpr(em, n.l);
      em.defMut(res, owned(l));
      const test = n.op === '&&' ? `vn_truth(&${res})`
        : n.op === '||' ? `!vn_truth(&${res})`
          : `${res}.is_nullish()`;
      em.line(`if ${test} {`);
      em.depth += 1;
      const r = emitExpr(em, n.r);
      em.set(res, owned(r));
      em.depth -= 1;
      em.line('}');
      return res;
    }
    case 'cond': {
      const res = em.tmp();
      em.defMut(res, 'Value::Undefined');
      const t = emitExpr(em, n.t);
      em.line(`if vn_truth(&${t}) {`);
      em.depth += 1;
      const c = emitExpr(em, n.c);
      em.set(res, owned(c));
      em.depth -= 1;
      em.line('} else {');
      em.depth += 1;
      const a = emitExpr(em, n.a);
      em.set(res, owned(a));
      em.depth -= 1;
      em.line('}');
      return res;
    }
    case 'seq': {
      let last = 'Value::Undefined';
      for (const e of n.es) last = emitExpr(em, e);
      return last;
    }
    case 'asgn':
      return emitAssign(em, n);
    case 'asgnp': {
      const v = emitExpr(em, n.v);
      bindPattern(em, n.pat, v);
      return v;
    }
    case 'logasgn':
      return emitLogAssign(em, n);
    case 'upd':
      return emitUpdate(em, n);
    case 'tmpl': {
      const s = em.uniq('s');
      em.line(`let mut ${s}: JsStr = JsStr::empty();`);
      for (const p of n.parts) {
        if (p.k === 'str') {
          em.line(`${s} = ${s}.concat(&JsStr::from_units(${em.strConst(p.u)}));`);
        } else {
          const x = emitExpr(em, p);
          const sv = em.async ? em.uniq('sv') : em.tmp();
          em.line(`let ${sv}: JsStr = rt.to_string(&${x})?;`);
          em.line(`${s} = ${s}.concat(&${sv});`);
        }
      }
      return bindTmp(em, `Value::String(${s})`);
    }
    case 'await': {
      if (!em.async) throw new Error('await outside an async function');
      const v = emitExpr(em, n.e);
      const r = em.tmp();
      em.def(r, `await_value(rt, ${owned(v)}).await?`);
      return r;
    }
    default:
      throw new Error(`emitExpr: unknown expression ${n.k}`);
  }
}

function calleeName(f) {
  if (f.k === 'mem') return f.key.k === 's' ? `.${f.key.s}` : '[computed]';
  if (f.k === 'var' || f.k === 'gvar') return f.name;
  return 'expression';
}

function emitCall(em, n) {
  const what = rstr(calleeName(n.f));
  if (n.f.k === 'mem') {
    const obj = emitExpr(em, n.f.o);
    const fv = getMember(em, obj, n.f.key);
    const args = emitArgs(em, n.args);
    return bindTmp(em, `vn_call(rt, &${fv}, &${owned(obj)}, &${args}, ${what})?`);
  }
  if (n.f.k === 'chain' && n.chainCallee) {
    const { v, t } = emitChain(em, n.f);
    const args = emitArgs(em, n.args);
    return bindTmp(em, `vn_call(rt, &${v}, &${t}.clone(), &${args}, ${what})?`);
  }
  const fv = emitExpr(em, n.f);
  const args = emitArgs(em, n.args);
  return bindTmp(em, `vn_call(rt, &${fv}, &Value::Undefined, &${args}, ${what})?`);
}

// Object literal: keys are converted before their values are evaluated (computed names), accessors merge.
function emitObject(em, n) {
  const o = em.objTmp();
  em.defObj(o, 'rt.new_object()');
  for (const p of n.props) {
    if (p.k === 'spread') {
      const x = emitExpr(em, p.e);
      em.line(`vn_copy_props(rt, ${o}, &${x}, &[])?;`);
      continue;
    }
    if (p.k === 'init') {
      if (p.key.k === 's') {
        const v = emitExpr(em, p.v);
        em.line(`rt.define_data(${o}, PropKey::lit(${rstr(p.key.s)}), ${owned(v)}, Flags::ALL)?;`);
      } else {
        const kv = emitExpr(em, p.key.e);
        const kk = em.keyTmp();
        em.defKey(kk, `vn_key(rt, &${kv})?`);
        const v = emitExpr(em, p.v);
        em.line(`rt.define_data(${o}, ${kk}.clone(), ${owned(v)}, Flags::ALL)?;`);
        if (p.v.k === 'fn' && em.hir.fns[p.v.f].nameFromKey) {
          em.line(`vn_set_name(rt, &${v}, &${kk}, "")?;`);
        }
      }
      continue;
    }
    // get / set: the key is converted first, then the function value is created.
    const kk = em.keyTmp();
    if (p.key.k === 's') em.defKey(kk, `PropKey::lit(${rstr(p.key.s)})`);
    else {
      const kv = emitExpr(em, p.key.e);
      em.defKey(kk, `vn_key(rt, &${kv})?`);
    }
    const f = emitExpr(em, p.v);
    if (p.v.k === 'fn' && em.hir.fns[p.v.f].nameFromKey) {
      em.line(`vn_set_name(rt, &${f}, &${kk}, ${rstr(p.k === 'get' ? 'get ' : 'set ')})?;`);
    }
    if (p.k === 'get') em.line(`vn_define_accessor(rt, ${o}, ${kk}.clone(), Some(${f}.clone()), None)?;`);
    else em.line(`vn_define_accessor(rt, ${o}, ${kk}.clone(), None, Some(${f}.clone()))?;`);
  }
  return bindTmp(em, `Value::Object(${o})`);
}
