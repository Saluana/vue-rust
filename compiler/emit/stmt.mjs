// Statement emission (WP-C1b). Loops are `'l<lid>: loop` with `'c<lid>: { body }` as the continue point, so a `continue`
// is a break of that block and the loop's update still runs. A jump that leaves a try/finally or a for-of body (both are
// Rust closures returning Res<Flow>) returns a Flow value; the code after the closure dispatches it in the enclosing
// context, where it becomes a Rust break/continue/return again or another Flow value. A for-of body also closes its
// iterator on every abrupt completion (ECMA-262 7.4.11).
import { declareScope, bindPattern, initVar } from './vars.mjs';
import { emitExpr } from './expr.mjs';
import { rstr } from './names.mjs';

const innermostClosure = (em) => {
  for (let i = em.ctl.length - 1; i >= 0; i--) if (em.ctl[i].type === 'closure') return i;
  return -1;
};

const findTarget = (em, lid) => {
  for (let i = em.ctl.length - 1; i >= 0; i--) {
    const e = em.ctl[i];
    if ((e.type === 'loop' || e.type === 'switch' || e.type === 'label') && e.lid === lid) return i;
  }
  throw new Error(`jump to unknown target ${lid}`);
};

// break / continue to a target in the current context (a Rust jump), or a Flow value out of the innermost closure.
export function emitJump(em, kind, lid) {
  const ti = findTarget(em, lid);
  const ci = innermostClosure(em);
  if (ci > ti) {
    const c = em.ctl[ci];
    if (kind === 'break') {
      c.pendingBreak.add(lid);
      em.line(`return Ok(Flow::Break(${lid}));`);
    } else {
      c.pendingCont.add(lid);
      em.line(`return Ok(Flow::Continue(${lid}));`);
    }
    return;
  }
  em.line(kind === 'break' ? `break 'l${lid};` : `break 'c${lid};`);
}

// return v (a Rust temporary) from the current context.
export function emitReturn(em, v) {
  const ci = innermostClosure(em);
  if (ci < 0) {
    em.line(`return Ok(${v}.clone());`);
  } else {
    em.ctl[ci].pendingRet = true;
    em.line(`return Ok(Flow::Return(${v}.clone()));`);
  }
}

function closureEntry(kind) {
  return { type: 'closure', kind, pendingBreak: new Set(), pendingCont: new Set(), pendingRet: false };
}

// Function declarations of a scope: created on entry, initialising their bindings.
export function emitHoist(em, hoist) {
  for (const h of hoist) {
    const f = emitExpr(em, h.fn);
    initVar(em, h.target.r, h.target.name, f);
  }
}

export function emitStatements(em, list) {
  for (const s of list) emitStmt(em, s);
}

export function emitStmt(em, s) {
  switch (s.k) {
    case 'expr':
      emitExpr(em, s.e);
      return;
    case 'decl': {
      const v = emitExpr(em, s.v);
      bindPattern(em, s.t, v);
      return;
    }
    case 'seqdecl':
      for (const d of s.list) emitStmt(em, d);
      return;
    case 'block':
      em.line('{');
      em.depth += 1;
      declareScope(em, s.scope.decls);
      emitHoist(em, s.scope.hoist);
      emitStatements(em, s.body);
      em.close('}');
      return;
    case 'empty':
      return;
    case 'if': {
      const t = emitExpr(em, s.t);
      em.line(`if vn_truth(&${t}) {`);
      em.depth += 1;
      emitStmt(em, s.c);
      em.depth -= 1;
      if (s.a) {
        em.line('} else {');
        em.depth += 1;
        emitStmt(em, s.a);
        em.depth -= 1;
      }
      em.line('}');
      return;
    }
    case 'while':
      return emitWhile(em, s);
    case 'dowhile':
      return emitDoWhile(em, s);
    case 'for':
      return emitFor(em, s);
    case 'forof':
      return emitForOf(em, s);
    case 'forin':
      return emitForIn(em, s);
    case 'switch':
      return emitSwitch(em, s);
    case 'try':
      return emitTry(em, s);
    case 'ret': {
      const v = s.e ? emitExpr(em, s.e) : 'Value::Undefined';
      emitReturn(em, v);
      return;
    }
    case 'throw': {
      const v = emitExpr(em, s.e);
      em.line(`return Err(Throw(${v}.clone()));`);
      return;
    }
    case 'break':
      emitJump(em, 'break', s.lid);
      return;
    case 'continue':
      emitJump(em, 'continue', s.lid);
      return;
    case 'label': {
      em.line(`'l${s.lid}: {`);
      em.depth += 1;
      em.ctl.push({ type: 'label', lid: s.lid });
      emitStmt(em, s.body);
      em.ctl.pop();
      em.close('}');
      return;
    }
    default:
      throw new Error(`emitStmt: unknown statement ${s.k}`);
  }
}

// Loop frame: the body is a labelled block so that `continue` (break 'c<lid>) reaches the update.
function loopBody(em, s, body) {
  em.ctl.push({ type: 'loop', lid: s.lid });
  em.line(`'c${s.lid}: {`);
  em.depth += 1;
  emitStmt(em, body);
  em.close('}');
  em.ctl.pop();
}

function emitWhile(em, s) {
  em.line(`'l${s.lid}: loop {`);
  em.depth += 1;
  em.line('rt.tick()?;');
  const t = emitExpr(em, s.t);
  em.line(`if !vn_truth(&${t}) { break 'l${s.lid}; }`);
  loopBody(em, s, s.body);
  em.close('}');
}

function emitDoWhile(em, s) {
  em.line(`'l${s.lid}: loop {`);
  em.depth += 1;
  em.line('rt.tick()?;');
  loopBody(em, s, s.body);
  const t = emitExpr(em, s.t);
  em.line(`if !vn_truth(&${t}) { break 'l${s.lid}; }`);
  em.close('}');
}

// for (init; test; update): one cell per iteration for captured lexical bindings (CreatePerIterationEnvironment copies
// the value before the first test and again after each body, so the update runs on the new copy).
function emitFor(em, s) {
  em.line('{');
  em.depth += 1;
  declareScope(em, s.scope.decls);
  if (s.init) emitStmt(em, s.init);
  const copies = () => {
    for (const id of s.scope.perIter) em.line(`c${id} = rt.new_cell(rt.cell_get(c${id}));`);
  };
  copies();
  em.line(`'l${s.lid}: loop {`);
  em.depth += 1;
  em.line('rt.tick()?;');
  if (s.t) {
    const t = emitExpr(em, s.t);
    em.line(`if !vn_truth(&${t}) { break 'l${s.lid}; }`);
  }
  loopBody(em, s, s.body);
  copies();
  if (s.upd) emitExpr(em, s.upd);
  em.close('}');
  em.close('}');
}

// for-in: the keys are snapshotted; each key is visited only if it is still present. No closure is needed: the loop has
// no iterator to close, so break and continue are plain Rust jumps.
function emitForIn(em, s) {
  const obj = em.tmp();
  const right = emitExpr(em, s.right);
  em.line(`let ${obj}: Option<ObjRef> = if ${right}.is_nullish() { None } else { Some(rt.to_object(&${right})?) };`);
  const keys = em.uniq('k');
  em.line(`let ${keys}: Vec<Value> = vn_for_in_keys(rt, &${right})?;`);
  em.line(`'l${s.lid}: for kv in ${keys} {`);
  em.depth += 1;
  em.line('rt.tick()?;');
  em.line(`if let Some(o) = ${obj} { if !vn_for_in_present(rt, o, &kv)? { continue; } }`);
  em.ctl.push({ type: 'loop', lid: s.lid });
  em.line(`'c${s.lid}: {`);
  em.depth += 1;
  emitForHead(em, s, 'kv');
  em.close('}');
  em.ctl.pop();
  em.close('}');
}

// Declares the scope of a for-in/of head, binds the loop variable (declaration or assignment target) and emits the body.
function emitForHead(em, s, valueName) {
  em.line('{');
  em.depth += 1;
  declareScope(em, s.scope.decls);
  bindPattern(em, s.left.t, valueName);
  emitStmt(em, s.body);
  em.close('}');
}

// for-of: the iterator is closed on break, return, an outer jump and a throw; continue to this loop just goes on. The body
// is a closure, so the loop's own target sits below it and every jump to the loop leaves the closure as a Flow value.
function emitForOf(em, s) {
  const right = emitExpr(em, s.right);
  const it = em.uniq('it');
  em.line(`let mut ${it} = get_iterator(rt, &${right})?;`);
  em.line(`'l${s.lid}: loop {`);
  em.depth += 1;
  em.line('rt.tick()?;');
  const v = em.uniq('v');
  em.line(`let ${v}: Value = match iter_step(rt, &mut ${it})? { Some(x) => x, None => break 'l${s.lid} };`);
  em.ctl.push({ type: 'loop', lid: s.lid });
  const clo = closureEntry('forof');
  const res = em.uniq('r');
  em.line(`let ${res}: Res<Flow> = (|| -> Res<Flow> {`);
  em.depth += 1;
  em.ctl.push(clo);
  emitForHead(em, s, v);
  em.ctl.pop();
  em.line('Ok(Flow::Normal)');
  em.depth -= 1;
  em.line('})();');
  em.line(`match ${res} {`);
  em.depth += 1;
  em.line(`Err(e) => { if !${it}.done { vn_iter_close_throw(rt, &${it}); } return Err(e); }`);
  em.line('Ok(Flow::Normal) => {}');
  em.line(`Ok(Flow::Continue(cid)) if cid == ${s.lid} => {}`);
  em.line(`Ok(Flow::Break(bid)) if bid == ${s.lid} => { vn_iter_close_normal(rt, &${it})?; break 'l${s.lid}; }`);
  em.line('Ok(other) => {');
  em.depth += 1;
  em.line(`vn_iter_close_normal(rt, &${it})?;`);
  emitDispatch(em, clo, 'other', s.lid);
  em.depth -= 1;
  em.line('}');
  em.depth -= 1;
  em.line('}');
  em.ctl.pop();
  em.close('}');
}

// Dispatches the completion `name` (a Flow value bound by the caller) after a closure, in the enclosing context.
function emitDispatch(em, clo, name, skipLid = null) {
  em.line(`match ${name} {`);
  em.depth += 1;
  em.line('Flow::Normal => {}');
  if (clo.pendingRet) {
    em.line('Flow::Return(rv) => {');
    em.depth += 1;
    emitReturn(em, 'rv');
    em.depth -= 1;
    em.line('}');
  } else {
    em.line('Flow::Return(_) => {}');
  }
  emitIdArms(em, 'Break', clo.pendingBreak, skipLid);
  emitIdArms(em, 'Continue', clo.pendingCont, skipLid);
  em.depth -= 1;
  em.line('}');
}

// The loop's own id (skipLid) is handled by the caller's guards; its jump needs no arm (a for-of has no continue block).
function emitIdArms(em, kind, set, skipLid = null) {
  set = new Set([...set].filter((lid) => lid !== skipLid));
  if (set.size === 0) {
    em.line(`Flow::${kind}(_) => {}`);
    return;
  }
  em.line(`Flow::${kind}(id) => match id {`);
  em.depth += 1;
  for (const lid of [...set].sort((a, b) => a - b)) {
    em.line(`${lid} => {`);
    em.depth += 1;
    emitJump(em, kind === 'Break' ? 'break' : 'continue', lid);
    em.depth -= 1;
    em.line('}');
  }
  em.line('_ => {}');
  em.depth -= 1;
  em.line('},');
}

// switch: the case tests run in source order (default skipped) until one matches; then the bodies run from that index
// on, so they fall through. Bodies are guarded by `idx <= i`.
function emitSwitch(em, s) {
  const d = emitExpr(em, s.disc);
  const dv = em.uniq('d');
  em.line(`let ${dv}: Value = ${d}.clone();`);
  em.line(`'l${s.lid}: {`);
  em.depth += 1;
  em.ctl.push({ type: 'switch', lid: s.lid });
  declareScope(em, s.scope.decls);
  emitHoist(em, s.scope.hoist);
  const idx = em.uniq('idx');
  em.line(`let mut ${idx}: usize = usize::MAX;`);
  let defaultIndex = -1;
  s.cases.forEach((c, i) => {
    if (c.t === null) {
      defaultIndex = i;
      return;
    }
    em.line(`if ${idx} == usize::MAX {`);
    em.depth += 1;
    const t = emitExpr(em, c.t);
    em.line(`if strict_eq(&${dv}, &${t}) { ${idx} = ${i}; }`);
    em.depth -= 1;
    em.line('}');
  });
  if (defaultIndex >= 0) {
    em.line(`if ${idx} == usize::MAX { ${idx} = ${defaultIndex}; }`);
  } else {
    em.line(`if ${idx} == usize::MAX { break 'l${s.lid}; }`);
  }
  s.cases.forEach((c, i) => {
    em.line(`if ${idx} <= ${i} {`);
    em.depth += 1;
    emitStatements(em, c.body);
    em.depth -= 1;
    em.line('}');
  });
  em.ctl.pop();
  em.close('}');
}

// try / catch / finally. Each part is a closure returning Res<Flow>; the completion is the try (or catch) result, unless
// the finally completes abruptly (then its completion wins). Any throw is an Err that the handler sees.
function emitTry(em, s) {
  const block = closureEntry('try');
  const r = em.uniq('r');
  em.line(`let mut ${r}: Res<Flow> = (|| -> Res<Flow> {`);
  em.depth += 1;
  em.ctl.push(block);
  emitStmt(em, s.block);
  em.ctl.pop();
  em.line('Ok(Flow::Normal)');
  em.depth -= 1;
  em.line('})();');
  const entries = [block];
  if (s.handler) {
    const catchClo = closureEntry('catch');
    entries.push(catchClo);
    const ex = em.uniq('x');
    em.line(`${r} = match ${r} {`);
    em.depth += 1;
    em.line(`Err(Throw(${ex})) => (|| -> Res<Flow> {`);
    em.depth += 1;
    em.ctl.push(catchClo);
    declareScope(em, s.handler.scope.decls);
    if (s.handler.param) bindPattern(em, s.handler.param, ex);
    emitHoist(em, s.handler.scope.hoist);
    emitStatements(em, s.handler.body);
    em.ctl.pop();
    em.line('Ok(Flow::Normal)');
    em.depth -= 1;
    em.line('})(),');
    em.line(`other => other,`);
    em.depth -= 1;
    em.line('};');
  }
  let completion = r;
  if (s.fin) {
    const finClo = closureEntry('finally');
    entries.push(finClo);
    const f = em.uniq('f');
    em.line(`let ${f}: Res<Flow> = (|| -> Res<Flow> {`);
    em.depth += 1;
    em.ctl.push(finClo);
    emitStmt(em, s.fin);
    em.ctl.pop();
    em.line('Ok(Flow::Normal)');
    em.depth -= 1;
    em.line('})();');
    completion = em.uniq('c');
    em.line(`let ${completion}: Res<Flow> = match ${f} { Ok(Flow::Normal) => ${r}, other => other };`);
  }
  // Merged pending jumps of every part (they all leave through the same completion).
  const merged = {
    pendingBreak: new Set(entries.flatMap((e) => [...e.pendingBreak])),
    pendingCont: new Set(entries.flatMap((e) => [...e.pendingCont])),
    pendingRet: entries.some((e) => e.pendingRet),
  };
  em.line(`match ${completion} {`);
  em.depth += 1;
  em.line('Err(e) => { return Err(e); }');
  em.line('Ok(Flow::Normal) => {}');
  emitDispatchArms(em, merged, completion);
  em.depth -= 1;
  em.line('}');
}

// The Flow arms of a completion match (the Err and Normal arms are written by the caller).
function emitDispatchArms(em, merged, name) {
  if (merged.pendingRet) {
    em.line('Ok(Flow::Return(rv)) => {');
    em.depth += 1;
    emitReturn(em, 'rv');
    em.depth -= 1;
    em.line('}');
  } else {
    em.line('Ok(Flow::Return(_)) => {}');
  }
  for (const [kind, set] of [['Break', merged.pendingBreak], ['Continue', merged.pendingCont]]) {
    if (set.size === 0) {
      em.line(`Ok(Flow::${kind}(_)) => {}`);
      continue;
    }
    em.line(`Ok(Flow::${kind}(id)) => match id {`);
    em.depth += 1;
    for (const lid of [...set].sort((a, b) => a - b)) {
      em.line(`${lid} => {`);
      em.depth += 1;
      emitJump(em, kind === 'Break' ? 'break' : 'continue', lid);
      em.depth -= 1;
      em.line('}');
    }
    em.line('_ => {}');
    em.depth -= 1;
    em.line('},');
  }
}

export { rstr };
