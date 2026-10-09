// Binding ownership, closure captures and storage for one module (WP-C1b). Input: the analyzeScopes result.
// A binding is owned by the function whose body (or parameter list, or named-expression scope) declares it; module
// bindings are owned by -1. Every function lists, in first-reference order, the outer bindings it reads or writes,
// transitively (a function that only passes a binding on to a nested closure still captures it). `this` of a non-arrow
// function is a pseudo binding "t:<fn>" that is captured when an arrow inside it uses `this`.
// Storage: module-scope bindings and imports are module cells; a binding captured by a closure is a heap cell; any other
// binding is a Rust local (blocks at module level included).

export function buildTables(analysis) {
  const { bindings, scopes, functions: fns, references } = analysis;
  const ownerCache = new Array(bindings.length);

  // The function that owns a binding. A named function expression's name belongs to that function.
  function ownerOf(bid) {
    if (ownerCache[bid] !== undefined) return ownerCache[bid];
    const b = bindings[bid];
    const s = scopes[b.scopeId];
    let owner = s.fnId;
    if (s.kind === 'funcname') {
      const fn = fns.find((f) => f.scopeId >= 0 && scopes[f.scopeId].parentId === s.id);
      owner = fn ? fn.id : s.fnId;
    }
    ownerCache[bid] = owner;
    return owner;
  }

  const captures = new Map(); // fn id -> string[] of "b:<bindingId>" | "t:<fnId>"
  const capSet = new Map();   // fn id -> Set (fast membership)
  const thisOwned = new Set(); // non-arrow fn ids whose `this` is read by an arrow

  function addCapture(fnId, key) {
    if (!capSet.has(fnId)) { capSet.set(fnId, new Set()); captures.set(fnId, []); }
    const set = capSet.get(fnId);
    if (!set.has(key)) { set.add(key); captures.get(fnId).push(key); }
  }

  function climb(fromFn, ownerFn, key) {
    let f = fromFn;
    while (f !== -1 && f !== ownerFn) {
      addCapture(f, key);
      f = fns[f].parentFnId;
    }
  }

  for (const r of references) {
    if (r.binding < 0) continue;
    climb(r.fnId, ownerOf(r.binding), `b:${r.binding}`);
  }
  for (const f of fns) {
    if (!f.isArrow || !f.usesThis || f.lexicalThisFnId < 0) continue;
    thisOwned.add(f.lexicalThisFnId);
    climb(f.id, f.lexicalThisFnId, `t:${f.lexicalThisFnId}`);
  }

  // Bindings owned by each function (module = -1), in binding id order.
  const ownedBy = new Map();
  for (const b of bindings) {
    const owner = ownerOf(b.id);
    if (!ownedBy.has(owner)) ownedBy.set(owner, []);
    ownedBy.get(owner).push(b.id);
  }

  // Module-scope bindings (and namespace imports) are module cells. A block at module level is not: it creates its
  // cells on each entry, like a block in a function, so a loop body gets a fresh binding per iteration.
  function storageOf(bid) {
    const b = bindings[bid];
    if (b.kind === 'import' && b.importedName !== '*') return 'import';
    if (scopes[b.scopeId].kind === 'module') return 'module';
    return b.captured ? 'cell' : 'local';
  }

  return {
    ownerOf,
    storageOf,
    captures: (fnId) => (captures.get(fnId) ?? []),
    ownedBy: (fnId) => (ownedBy.get(fnId) ?? []),
    thisOwned: (fnId) => thisOwned.has(fnId),
  };
}
