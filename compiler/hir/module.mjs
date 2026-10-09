// Module lowering (WP-C1b): the module record of the HIR. Module-scope bindings are numbered in analysis order
// (`slots`); the emitter creates one cell per slot in `declare`. Imports and re-exports refer to other modules by their
// canonical request ids (cx.resolve). Top-level statements form the `body` (evaluate phase); function declarations are
// the `hoist` list (bind phase). `export default <expr>` writes a hidden binding `*default*`.
import { hn, unsupported, declaredBinding } from './context.mjs';
import { scopeEntry, lowerStatement, functionDeclOf } from './stmt.mjs';
import { lowerExpr, lowerNamed } from './expr.mjs';
import { lowerFunctionValue } from './fn.mjs';
import { isAnonymousFunction } from './literals.mjs';

function moduleNameOf(node) {
  return node.type === 'Identifier' ? node.name : node.value;
}

// Names bound by an exported declaration (variables, functions), in source order.
function declaredNames(decl) {
  const names = [];
  const collect = (p) => {
    if (!p) return;
    if (p.type === 'Identifier') names.push(p.name);
    else if (p.type === 'ObjectPattern') p.properties.forEach((q) => collect(q.type === 'RestElement' ? q.argument : q.value));
    else if (p.type === 'ArrayPattern') p.elements.forEach(collect);
    else if (p.type === 'AssignmentPattern') collect(p.left);
    else if (p.type === 'RestElement') collect(p.argument);
  };
  if (decl.type === 'VariableDeclaration') decl.declarations.forEach((d) => collect(d.id));
  else if (decl.id) names.push(decl.id.name);
  return names;
}

export function lowerModuleAst(cx, { specifier, origin, lang }) {
  const program = cx.program;
  const modScope = cx.analysis.module.id;
  const requests = [];
  const addRequest = (src) => {
    const r = cx.resolve(src);
    if (!requests.includes(r)) requests.push(r);
    return r;
  };
  const imports = [];
  const exportsLocal = [];
  const exportsIndirect = [];
  const exportsStar = [];
  const exportsNs = [];
  const nsImports = [];
  const body = [];
  const hoistDefault = [];
  const syntheticBindings = [];
  const nsOf = new Map(); // local name -> request of an `import * as` binding

  // In TypeScript an import binding that no value position reads is a type (the analysis records value references only),
  // so the specifier is erased; an import whose specifiers are all erased is removed with its request (TS elision).
  const valueRefs = lang === 'ts' ? new Set(cx.analysis.references.filter((r) => r.binding >= 0).map((r) => r.binding)) : null;
  const isTypeOnly = (x) => x.importKind === 'type' || x.importKind === 'typeof';

  // Pass 1: imports and export clauses (bindings and requests in source order).
  for (const s of program.body) {
    if (s.type === 'ImportDeclaration') {
      if (s.importKind === 'type' || s.importKind === 'typeof') continue;
      const kept = s.specifiers.filter((spec) => !isTypeOnly(spec) &&
        (valueRefs === null || valueRefs.has(declaredBinding(cx, modScope, spec.local.name, spec))));
      if (s.specifiers.length > 0 && kept.length === 0) continue;
      const request = addRequest(s.source.value);
      for (const spec of kept) {
        if (spec.importKind === 'type' || spec.importKind === 'typeof') continue;
        const local = spec.local.name;
        const bid = declaredBinding(cx, modScope, local, spec);
        if (spec.type === 'ImportNamespaceSpecifier') {
          nsImports.push({ id: bid, name: local, request });
          nsOf.set(local, request);
          continue;
        }
        const imported = spec.type === 'ImportDefaultSpecifier' ? 'default' : moduleNameOf(spec.imported);
        imports.push({ id: bid, name: local, request, imported });
      }
    } else if (s.type === 'ExportAllDeclaration') {
      if (s.exportKind === 'type') continue;
      exportsStar.push(addRequest(s.source.value));
    } else if (s.type === 'ExportNamedDeclaration') {
      if (s.exportKind === 'type') continue;
      if (s.source) {
        if (s.specifiers.length > 0 && s.specifiers.every((x) => x.exportKind === 'type')) continue;
        const request = addRequest(s.source.value);
        for (const spec of s.specifiers) {
          if (spec.exportKind === 'type') continue;
          if (spec.type === 'ExportNamespaceSpecifier') exportsNs.push({ name: moduleNameOf(spec.exported), request });
          else if (spec.type === 'ExportSpecifier') {
            exportsIndirect.push({ name: moduleNameOf(spec.exported), request, imported: moduleNameOf(spec.local) });
          }
        }
      } else if (s.declaration) {
        if (s.declaration.declare) continue;
        if (s.declaration.type === 'TSInterfaceDeclaration' || s.declaration.type === 'TSTypeAliasDeclaration') continue;
        for (const name of declaredNames(s.declaration)) {
          exportsLocal.push({ name, id: declaredBinding(cx, modScope, name, s.declaration) });
        }
      } else {
        for (const spec of s.specifiers) {
          if (spec.exportKind === 'type') continue;
          const local = spec.local.name;
          const exported = moduleNameOf(spec.exported);
          if (nsOf.has(local)) {
            // `import * as ns; export { ns }`: a local export of the namespace binding.
            exportsLocal.push({ name: exported, id: declaredBinding(cx, modScope, local, spec) });
            continue;
          }
          const importEntry = imports.find((i) => i.name === local);
          if (importEntry) {
            exportsIndirect.push({ name: exported, request: importEntry.request, imported: importEntry.imported });
            continue;
          }
          exportsLocal.push({ name: exported, id: declaredBinding(cx, modScope, local, spec) });
        }
      }
    } else if (s.type === 'ExportDefaultDeclaration') {
      const decl = s.declaration;
      if (decl.type === 'ClassDeclaration') unsupported(cx, decl, 'class declaration');
      if (decl.type === 'FunctionDeclaration' && decl.id) {
        exportsLocal.push({ name: 'default', id: declaredBinding(cx, modScope, decl.id.name, decl) });
      } else {
        const syn = cx.analysis.bindings.length + syntheticBindings.length;
        syntheticBindings.push({ id: syn, name: '*default*', kind: 'const' });
        exportsLocal.push({ name: 'default', id: syn });
      }
    }
  }

  // Pass 2: the scope entry of the module (declarations and hoisted functions), then the body.
  const entry = scopeEntry(cx, modScope, program.body);
  for (const s of program.body) {
    if (s.type === 'ExportDefaultDeclaration' && s.declaration.type === 'FunctionDeclaration' && !s.declaration.id) {
      const syn = syntheticBindings.find((b) => b.name === '*default*');
      const fn = lowerFunctionValue(cx, s.declaration, { name: 'default', decl: true });
      hoistDefault.push({ id: syn.id, target: { k: 'pinit', id: syn.id, r: { k: 'M', id: syn.id }, tdz: false, name: '*default*' }, fn });
      continue;
    }
    if (functionDeclOf(s)) continue;
    if (s.type === 'ImportDeclaration' || s.type === 'ExportAllDeclaration') continue;
    if (s.type === 'ExportNamedDeclaration') {
      if (s.source || s.declaration === null || s.declaration === undefined) continue;
      if (s.declaration.declare) continue;
      const out = lowerStatement(cx, s.declaration, modScope);
      if (out) body.push(out);
      continue;
    }
    if (s.type === 'ExportDefaultDeclaration') {
      const decl = s.declaration;
      if (decl.type === 'FunctionDeclaration') continue;
      const syn = syntheticBindings.find((b) => b.name === '*default*');
      const value = isAnonymousFunction(decl) ? lowerNamed(cx, decl, 'default') : lowerExpr(cx, decl);
      body.push(hn('decl', s, { t: { k: 'pinit', id: syn.id, r: { k: 'M', id: syn.id }, tdz: false, name: '*default*' }, v: value }));
      continue;
    }
    const out = lowerStatement(cx, s, modScope);
    if (out !== null) body.push(out);
  }

  // Module-scope bindings in analysis order (imports excluded: they are read through the exporter's cells).
  const slots = [];
  for (const b of cx.analysis.bindings) {
    if (b.kind === 'import' && b.importedName !== '*') continue;
    if (cx.scopes[b.scopeId].kind !== 'module') continue;
    slots.push({ id: b.id, name: b.name, kind: b.kind, init: initOf(b.kind), tdz: b.needsTdzCheck });
  }
  for (const ns of nsImports) {
    slots.push({ id: ns.id, name: ns.name, kind: 'nsimport', init: 'ns', request: ns.request });
  }
  for (const syn of syntheticBindings) {
    slots.push({ id: syn.id, name: syn.name, kind: syn.kind, init: 'uninit', tdz: true });
  }
  slots.sort((a, b) => a.id - b.id);

  const hoist = [...entry.hoist, ...hoistDefault];
  return {
    hirVersion: 1,
    kind: 'Module',
    specifier,
    origin,
    lang,
    requests,
    slots,
    imports,
    exportsLocal,
    exportsIndirect,
    exportsStar,
    exportsNs,
    hoist,
    body,
    fns: cx.fnRecords,
  };
}

// Initial state of a module cell: `var` starts undefined, `let`/`const`/functions start uninitialised.
function initOf(kind) {
  if (kind === 'var') return 'undef';
  return 'uninit';
}

