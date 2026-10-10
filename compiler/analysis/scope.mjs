// Scope, hoisting, TDZ and closure analysis over a Babel AST (module code, strict mode).
// Output is JSON-serializable: scopes, bindings, references and functions use numeric ids (index = id).
//
// Documented approximations:
// - Default-value TDZ between parameters is handled by the textual rule (a read before the parameter's end is a TDZ read).
// - Lexical initialization completes at the end of its declarator (for-in/of heads: at the end of the iterable).
// - needsTdzCheck is conservative (no false negatives): a read or write textually before initialization completes,
//   a read from a closure created before initialization completes (a function declaration counts as created at the
//   start of its containing scope), any read of a switch-case lexical binding, and every import binding (a cyclic
//   module graph can read an uninitialized live binding).
// - Types are erased. Type annotations, interfaces, type aliases and declare-d names create no bindings or
//   references. Named type-only imports create nothing.
// - Class declarations are bound like let. Class bodies get method and field function scopes. Classes are rejected by
//   the profile checker, so this path only has to be safe, not complete.
// - Writes to const, import and named function-expression bindings produce warnings (VN5002..VN5004); the runtime
//   TypeError is the behaviour, the warning is the static finding.
import { createDiagnostic, createLineMap } from '../diagnostics/diagnostic.mjs';

const TDZ_KINDS = new Set(['let', 'const', 'param']);
const SKIP_CHILD_KEYS = new Set([
  'type', 'start', 'end', 'loc', 'range', 'extra', 'leadingComments', 'trailingComments', 'innerComments',
  'comments', 'typeAnnotation', 'returnType', 'typeParameters', 'superTypeParameters', 'implements', 'predicate',
  'decorators',
]);

function isNode(value) {
  return value !== null && typeof value === 'object' && typeof value.type === 'string';
}

// A TypeScript `this` parameter (`function f(this: T, ...)`) is an Identifier named `this`, which no JavaScript binding can
// be. TypeScript erases it: it is no parameter, so it binds nothing, counts for no `length` and takes no argument. The
// lowering (compiler/hir/fn.mjs) skips it for the same reason.
export function isThisParameter(node) {
  return node.type === 'Identifier' && node.name === 'this';
}

export function analyzeScopes(ast, { source, filename = '<input>' } = {}) {
  if (typeof source !== 'string') throw new TypeError('analyzeScopes: options.source must be the source string');
  if (!isNode(ast)) throw new TypeError('analyzeScopes: expected a Babel File or Program node');
  const program = ast.type === 'File' ? ast.program : ast;
  if (!isNode(program) || program.type !== 'Program') throw new TypeError('analyzeScopes: expected a Program node');

  const lines = createLineMap(source);
  const scopes = [];
  const fns = [];
  const bindings = [];
  const refs = [];
  const exportPending = [];
  const diags = [];
  const fnStack = [];
  let moduleUsesThis = false;

  function addDiag(code, start, end, message, severity = 'error') {
    diags.push(createDiagnostic(code, {
      message,
      severity,
      file: filename,
      start: lines.posAt(start),
      end: lines.posAt(end),
    }));
  }

  // fnId: the function record that owns the scope; defaults to the innermost active function.
  function newScope(kind, parent, start, end, fnId = currentFnId()) {
    const s = {
      id: scopes.length,
      kind,
      parentId: parent ? parent.id : -1,
      start,
      end,
      fnId,
      names: new Map(),
      decls: [],
      through: [],
      bindingIds: [],
      childIds: [],
    };
    scopes.push(s);
    if (parent) parent.childIds.push(s.id);
    return s;
  }

  function currentFnId() {
    return fnStack.length ? fnStack[fnStack.length - 1].id : -1;
  }

  // Declares one identifier in scope. info: { kind, cat: 'LEX'|'VAR'|'PARAM', initEnd, declStart, declEnd,
  // forKind, importedName, source, selfName, throughScopes }.
  function declare(scope, id, info) {
    const name = id.name;
    if (info.cat === 'VAR' && info.throughScopes) {
      for (const t of info.throughScopes) t.through.push({ name, start: id.start, end: id.end });
    }
    scope.decls.push({ name, cat: info.cat, start: id.start, end: id.end });
    let b = scope.names.get(name);
    if (!b) {
      b = {
        id: bindings.length,
        name,
        kind: info.kind,
        scopeId: scope.id,
        start: id.start,
        end: id.end,
        declStart: info.declStart ?? id.start,
        declEnd: info.declEnd ?? id.end,
        captured: false,
        mutated: false,
        needsTdzCheck: info.kind === 'import',
        perIterationCopy: false,
        exported: [],
        initEnd: info.initEnd ?? -1,
        forKind: info.forKind ?? null,
        inSwitch: scope.kind === 'switch' && (info.kind === 'let' || info.kind === 'const'),
        selfName: !!info.selfName,
        importedName: info.importedName ?? null,
        source: info.source ?? null,
      };
      bindings.push(b);
      scope.names.set(name, b);
      scope.bindingIds.push(b.id);
    }
    return b;
  }

  // var declarations hoist to the nearest function or module scope; block scopes passed on the way record the
  // name so a lexical declaration of the same name in those blocks is reported as a conflict.
  function varTarget(scope) {
    const through = [];
    let s = scope;
    while (s.kind !== 'function' && s.kind !== 'function-body' && s.kind !== 'module') {
      through.push(s);
      s = scopes[s.parentId];
    }
    return { target: s, through };
  }

  function addRef(id, scope, { read = true, write = false, typeofOnly = false } = {}) {
    refs.push({
      id: refs.length,
      name: id.name,
      start: id.start,
      end: id.end,
      scopeId: scope.id,
      fnId: currentFnId(),
      binding: -1,
      read,
      write,
      typeofOnly,
      crossesFunction: false,
    });
  }

  function newFn(kind, node, scope, { isArrow, hoisted, creationStart }) {
    const parent = fnStack.length ? fnStack[fnStack.length - 1] : null;
    let lexicalThisFnId = -1;
    if (isArrow) {
      for (let i = fnStack.length - 1; i >= 0; i--) {
        if (!fnStack[i].isArrow) { lexicalThisFnId = fnStack[i].id; break; }
      }
    }
    const f = {
      id: fns.length,
      kind,
      isArrow,
      async: !!node.async,
      generator: !!node.generator,
      name: node.id && node.id.type === 'Identifier' ? node.id.name : null,
      start: node.start,
      end: node.end,
      scopeId: -1,
      parentFnId: parent ? parent.id : -1,
      hoisted,
      creationStart,
      usesThis: false,
      capturesThis: false,
      usesArguments: false,
      lexicalThisFnId: isArrow ? lexicalThisFnId : -1,
    };
    fns.push(f);
    return f;
  }

  function noteThis() {
    if (fnStack.length === 0) { moduleUsesThis = true; return; }
    const inner = fnStack[fnStack.length - 1];
    inner.usesThis = true;
    if (inner.isArrow) {
      let owner = null;
      for (let i = fnStack.length - 1; i >= 0; i--) {
        if (!fnStack[i].isArrow) { owner = fnStack[i]; break; }
      }
      if (owner) owner.capturesThis = true;
      else moduleUsesThis = true;
    }
  }

  // Binds a declaration pattern. info is copied per identifier; defaults and computed keys are read references.
  function bindPattern(node, scope, info) {
    if (!node) return;
    switch (node.type) {
      case 'Identifier':
        declare(scope, node, info);
        return;
      case 'ObjectPattern':
        for (const prop of node.properties) {
          if (prop.type === 'RestElement') bindPattern(prop.argument, scope, info);
          else {
            if (prop.computed) visit(prop.key, scope);
            bindPattern(prop.value, scope, info);
          }
        }
        return;
      case 'ArrayPattern':
        for (const el of node.elements) bindPattern(el, scope, info);
        return;
      case 'AssignmentPattern':
        bindPattern(node.left, scope, info);
        visit(node.right, scope);
        return;
      case 'RestElement':
        bindPattern(node.argument, scope, info);
        return;
      case 'TSParameterProperty':
        bindPattern(node.parameter, scope, info);
        return;
      default:
        // Not a binding form (should not occur in valid code): treat as an assignment target.
        visitTarget(node, scope, false);
    }
  }

  // Assignment targets: identifiers are writes; member expressions and destructuring defaults are reads.
  function visitTarget(node, scope, compound) {
    if (!node) return;
    switch (node.type) {
      case 'Identifier':
        addRef(node, scope, { read: compound, write: true });
        return;
      case 'MemberExpression':
      case 'OptionalMemberExpression':
        visit(node.object, scope);
        if (node.computed) visit(node.property, scope);
        return;
      case 'ObjectPattern':
        for (const prop of node.properties) {
          if (prop.type === 'RestElement') visitTarget(prop.argument, scope, false);
          else {
            if (prop.computed) visit(prop.key, scope);
            visitTarget(prop.value, scope, false);
          }
        }
        return;
      case 'ArrayPattern':
        for (const el of node.elements) visitTarget(el, scope, false);
        return;
      case 'AssignmentPattern':
        visitTarget(node.left, scope, false);
        visit(node.right, scope);
        return;
      case 'RestElement':
        visitTarget(node.argument, scope, false);
        return;
      case 'TSAsExpression':
      case 'TSSatisfiesExpression':
      case 'TSNonNullExpression':
      case 'TSTypeAssertion':
      case 'ParenthesizedExpression':
        visitTarget(node.expression, scope, compound);
        return;
      default:
        visit(node, scope);
    }
  }

  function visitStatements(list, scope) {
    for (const s of list) visit(s, scope);
  }

  function visitBlock(node, scope) {
    const block = newScope('block', scope, node.start, node.end);
    visitStatements(node.body, block);
  }

  // Statement positions that may hold a non-block statement (if/loop bodies, labels).
  function visitBody(node, scope) {
    if (!node) return;
    if (node.type === 'BlockStatement') visitBlock(node, scope);
    else visit(node, scope);
  }

  function visitVariableDeclaration(node, scope, forKind = null) {
    if (node.declare) return;
    const isVar = node.kind === 'var';
    for (const d of node.declarations) {
      if (isVar) {
        const { target, through } = varTarget(scope);
        bindPattern(d.id, target, {
          kind: 'var',
          cat: 'VAR',
          throughScopes: through,
          declStart: d.start,
          declEnd: d.end,
          forKind,
        });
      } else {
        // for-in/of heads are handled by the ForIn/ForOf case; here initialization completes at the declarator end.
        bindPattern(d.id, scope, {
          kind: node.kind === 'let' ? 'let' : 'const',
          cat: 'LEX',
          initEnd: d.end,
          declStart: d.start,
          declEnd: d.end,
          forKind,
        });
      }
      if (d.init) visit(d.init, scope);
    }
  }

  function visitFunctionBody(node, fnScope) {
    if (node.body && node.body.type === 'BlockStatement') visitStatements(node.body.body, fnScope);
    else visit(node.body, fnScope);
  }

  // Function-like nodes: declarations, expressions, arrows, object/class methods and accessors.
  function visitFunction(node, scope, { kind, hoisted = false, creationStart = node.start }) {
    const isArrow = node.type === 'ArrowFunctionExpression';
    let outer = scope;
    if (node.type === 'FunctionExpression' && node.id) {
      outer = newScope('funcname', scope, node.start, node.end);
      declare(outer, node.id, { kind: 'function', cat: 'VAR', selfName: true, declStart: node.start, declEnd: node.end });
    }
    const fn = newFn(kind, node, outer, { isArrow, hoisted, creationStart });
    const bodyStart = node.body ? node.body.start : node.start;
    const fnScope = newScope('function', outer, bodyStart, node.end, fn.id);
    fn.scopeId = fnScope.id;
    fnStack.push(fn);
    for (const p of node.params) {
      if (isThisParameter(p)) continue;
      bindPattern(p, fnScope, { kind: 'param', cat: 'PARAM', initEnd: p.end, declStart: p.start, declEnd: p.end });
    }
    const parameterExpression = p => {
      if (p.type === 'AssignmentPattern') return true;
      if (p.type === 'RestElement') return parameterExpression(p.argument);
      if (p.type === 'ArrayPattern') return p.elements.some(e => e && parameterExpression(e));
      if (p.type === 'ObjectPattern') return p.properties.some(prop => prop.computed || parameterExpression(prop.value ?? prop.argument));
      return false;
    };
    const hasExpressions = node.params.some(parameterExpression);
    const bodyScope = hasExpressions && node.body.type === 'BlockStatement'
      ? newScope('function-body', fnScope, bodyStart, node.end, fn.id) : fnScope;
    fn.bodyScopeId = bodyScope.id;
    visitFunctionBody(node, bodyScope);
    fnStack.pop();
    return fn;
  }

  function visitClassBody(body, scope) {
    for (const member of body.body) {
      if (member.type === 'ClassMethod' || member.type === 'ClassPrivateMethod') {
        if (member.computed) visit(member.key, scope);
        const kind = member.kind === 'get' ? 'getter' : member.kind === 'set' ? 'setter' : 'method';
        visitFunction(member, scope, { kind });
      } else if (
        member.type === 'ClassProperty' || member.type === 'ClassPrivateProperty' || member.type === 'ClassAccessorProperty'
      ) {
        if (member.computed) visit(member.key, scope);
        if (member.value) {
          const fn = newFn('field', member, scope, { isArrow: false, hoisted: false, creationStart: member.start });
          const fieldScope = newScope('function', scope, member.start, member.end, fn.id);
          fn.scopeId = fieldScope.id;
          fnStack.push(fn);
          visit(member.value, fieldScope);
          fnStack.pop();
        }
      } else if (member.type === 'StaticBlock') {
        const fn = newFn('static-block', member, scope, { isArrow: false, hoisted: false, creationStart: member.start });
        const blockScope = newScope('function', scope, member.start, member.end, fn.id);
        fn.scopeId = blockScope.id;
        fnStack.push(fn);
        visitStatements(member.body, blockScope);
        fnStack.pop();
      }
    }
  }

  function visitModuleSpecifierName(node) {
    return node.type === 'Identifier' ? node.name : node.value;
  }

  function visitImport(node, scope) {
    if (node.importKind === 'type' || node.importKind === 'typeof') return;
    for (const spec of node.specifiers) {
      if (spec.importKind === 'type' || spec.importKind === 'typeof') continue;
      let importedName;
      if (spec.type === 'ImportDefaultSpecifier') importedName = 'default';
      else if (spec.type === 'ImportNamespaceSpecifier') importedName = '*';
      else importedName = visitModuleSpecifierName(spec.imported);
      declare(scope, spec.local, {
        kind: 'import',
        cat: 'LEX',
        importedName,
        source: node.source.value,
        declStart: spec.start,
        declEnd: spec.end,
      });
    }
  }

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
    else if (decl.id && decl.id.type === 'Identifier') names.push(decl.id.name);
    return names;
  }

  function visit(node, scope) {
    if (!node) return;
    switch (node.type) {
      case 'Identifier':
        addRef(node, scope, { read: true });
        return;

      case 'ThisExpression':
        noteThis();
        return;

      case 'Super':
      case 'Import':
      case 'MetaProperty':
      case 'PrivateName':
      case 'EmptyStatement':
      case 'DebuggerStatement':
      case 'BreakStatement':
      case 'ContinueStatement':
      case 'StringLiteral':
      case 'NumericLiteral':
      case 'BigIntLiteral':
      case 'BooleanLiteral':
      case 'NullLiteral':
      case 'RegExpLiteral':
      case 'TSNamespaceExportDeclaration':
      case 'TSInterfaceDeclaration':
      case 'TSTypeAliasDeclaration':
      case 'TSDeclareFunction':
      case 'TSDeclareMethod':
      case 'ExportAllDeclaration':
        // Declarations of types and ambient functions create no runtime bindings; labels are not references.
        return;

      case 'TemplateLiteral':
        for (const e of node.expressions) visit(e, scope);
        return;

      case 'TaggedTemplateExpression':
        visit(node.tag, scope);
        visit(node.quasi, scope);
        return;

      case 'ArrayExpression':
        for (const el of node.elements) visit(el, scope);
        return;

      case 'ObjectExpression':
        for (const prop of node.properties) {
          if (prop.type === 'SpreadElement') visit(prop.argument, scope);
          else if (prop.type === 'ObjectMethod') {
            if (prop.computed) visit(prop.key, scope);
            const kind = prop.kind === 'get' ? 'getter' : prop.kind === 'set' ? 'setter' : 'method';
            visitFunction(prop, scope, { kind });
          } else {
            if (prop.computed) visit(prop.key, scope);
            if (prop.shorthand && prop.value.type === 'Identifier') addRef(prop.value, scope, { read: true });
            else visit(prop.value, scope);
          }
        }
        return;

      case 'SpreadElement':
      case 'ParenthesizedExpression':
      case 'Decorator':
      case 'AwaitExpression':
      case 'YieldExpression':
        visit(node.argument ?? node.expression, scope);
        return;

      case 'UnaryExpression':
        if (node.operator === 'typeof' && node.argument.type === 'Identifier') {
          addRef(node.argument, scope, { read: true, typeofOnly: true });
        } else {
          visit(node.argument, scope);
        }
        return;

      case 'UpdateExpression':
        visitTarget(node.argument, scope, true);
        return;

      case 'BinaryExpression':
      case 'LogicalExpression':
        visit(node.left, scope);
        visit(node.right, scope);
        return;

      case 'AssignmentExpression':
        visitTarget(node.left, scope, node.operator !== '=');
        visit(node.right, scope);
        return;

      case 'ConditionalExpression':
        visit(node.test, scope);
        visit(node.consequent, scope);
        visit(node.alternate, scope);
        return;

      case 'SequenceExpression':
        for (const e of node.expressions) visit(e, scope);
        return;

      case 'CallExpression':
      case 'OptionalCallExpression':
        if (node.callee.type !== 'Import') visit(node.callee, scope);
        for (const a of node.arguments) visit(a, scope);
        return;

      case 'ImportExpression':
        visit(node.source, scope);
        if (node.options) visit(node.options, scope);
        return;

      case 'NewExpression':
        visit(node.callee, scope);
        for (const a of node.arguments) visit(a, scope);
        return;

      case 'MemberExpression':
      case 'OptionalMemberExpression':
        visit(node.object, scope);
        if (node.computed) visit(node.property, scope);
        return;

      case 'ArrowFunctionExpression':
      case 'FunctionExpression':
        visitFunction(node, scope, { kind: node.type === 'ArrowFunctionExpression' ? 'arrow' : 'expression' });
        return;

      case 'ClassExpression': {
        visit(node.superClass, scope);
        let inner = scope;
        if (node.id) {
          inner = newScope('funcname', scope, node.start, node.end);
          declare(inner, node.id, { kind: 'const', cat: 'VAR', selfName: true, declStart: node.start, declEnd: node.end });
        }
        visitClassBody(node.body, inner);
        return;
      }

      case 'ClassDeclaration': {
        if (node.declare) return;
        if (node.id) {
          declare(scope, node.id, { kind: 'let', cat: 'LEX', initEnd: node.end, declStart: node.start, declEnd: node.end });
        }
        visit(node.superClass, scope);
        visitClassBody(node.body, scope);
        return;
      }

      case 'FunctionDeclaration':
        visitFunctionDeclaration(node, scope, { exportName: null });
        return;

      case 'VariableDeclaration':
        visitVariableDeclaration(node, scope);
        return;

      case 'ExpressionStatement':
        visit(node.expression, scope);
        return;

      case 'ReturnStatement':
      case 'ThrowStatement':
        visit(node.argument, scope);
        return;

      case 'BlockStatement':
        visitBlock(node, scope);
        return;

      case 'IfStatement':
        visit(node.test, scope);
        visitBody(node.consequent, scope);
        visitBody(node.alternate, scope);
        return;

      case 'WhileStatement':
        visit(node.test, scope);
        visitBody(node.body, scope);
        return;

      case 'DoWhileStatement':
        visitBody(node.body, scope);
        visit(node.test, scope);
        return;

      case 'ForStatement': {
        const forScope = newScope('for', scope, node.start, node.end);
        if (node.init) {
          if (node.init.type === 'VariableDeclaration') visitVariableDeclaration(node.init, forScope, 'c-style');
          else visit(node.init, forScope);
        }
        visit(node.test, forScope);
        visit(node.update, forScope);
        visitBody(node.body, forScope);
        return;
      }

      case 'ForInStatement':
      case 'ForOfStatement': {
        const forScope = newScope('for', scope, node.start, node.end);
        const left = node.left;
        if (left.type === 'VariableDeclaration') {
          for (const d of left.declarations) {
            if (left.kind === 'var') {
              const { target, through } = varTarget(forScope);
              bindPattern(d.id, target, {
                kind: 'var',
                cat: 'VAR',
                throughScopes: through,
                declStart: d.start,
                declEnd: d.end,
                forKind: 'in-of',
              });
            } else {
              // Head bindings are in TDZ while the iterable is evaluated: initialization completes after it.
              bindPattern(d.id, forScope, {
                kind: left.kind === 'let' ? 'let' : 'const',
                cat: 'LEX',
                initEnd: node.right.end,
                declStart: d.start,
                declEnd: d.end,
                forKind: 'in-of',
              });
            }
          }
        } else {
          visitTarget(left, forScope, false);
        }
        visit(node.right, forScope);
        visitBody(node.body, forScope);
        return;
      }

      case 'SwitchStatement': {
        visit(node.discriminant, scope);
        const switchScope = newScope('switch', scope, node.start, node.end);
        for (const c of node.cases) {
          visit(c.test, switchScope);
          visitStatements(c.consequent, switchScope);
        }
        return;
      }

      case 'TryStatement':
        visitBlock(node.block, scope);
        if (node.handler) {
          const catchScope = newScope('catch', scope, node.handler.start, node.handler.end);
          if (node.handler.param) {
            bindPattern(node.handler.param, catchScope, {
              kind: 'catch',
              cat: 'PARAM',
              initEnd: -1,
              declStart: node.handler.param.start,
              declEnd: node.handler.param.end,
            });
          }
          visitStatements(node.handler.body.body, catchScope);
        }
        if (node.finalizer) visitBlock(node.finalizer, scope);
        return;

      case 'LabeledStatement':
        visitBody(node.body, scope);
        return;

      case 'WithStatement':
        visit(node.object, scope);
        visitBody(node.body, scope);
        return;

      case 'ImportDeclaration':
        visitImport(node, scope);
        return;

      case 'ExportNamedDeclaration': {
        if (node.exportKind === 'type') return;
        if (node.declaration) {
          visit(node.declaration, scope);
          const runtime = ['VariableDeclaration', 'FunctionDeclaration', 'ClassDeclaration', 'TSEnumDeclaration', 'TSModuleDeclaration'];
          if (runtime.includes(node.declaration.type) && !node.declaration.declare) {
            for (const name of declaredNames(node.declaration)) exportPending.push({ local: name, exported: name });
          }
          return;
        }
        if (node.source) return;
        for (const spec of node.specifiers) {
          if (spec.type !== 'ExportSpecifier' || spec.exportKind === 'type') continue;
          addRef(spec.local, scope, { read: true });
          exportPending.push({ local: spec.local.name, exported: visitModuleSpecifierName(spec.exported) });
        }
        return;
      }

      case 'ExportDefaultDeclaration': {
        const decl = node.declaration;
        if (decl.type === 'FunctionDeclaration') {
          if (decl.id) {
            visitFunctionDeclaration(decl, scope, { exportName: 'default' });
          } else {
            visitFunction(decl, scope, { kind: 'declaration', hoisted: true, creationStart: scope.start });
          }
        } else if (decl.type === 'ClassDeclaration') {
          if (decl.id) exportPending.push({ local: decl.id.name, exported: 'default' });
          visit(decl, scope);
        } else {
          visit(decl, scope);
        }
        return;
      }

      case 'TSEnumDeclaration': {
        if (node.declare) return;
        declare(scope, node.id, { kind: 'let', cat: 'LEX', initEnd: node.end, declStart: node.start, declEnd: node.end });
        for (const m of node.members) visit(m.initializer, scope);
        return;
      }

      case 'TSModuleDeclaration': {
        if (node.declare) return;
        if (node.id.type === 'Identifier') {
          declare(scope, node.id, { kind: 'let', cat: 'LEX', initEnd: node.end, declStart: node.start, declEnd: node.end });
        }
        if (node.body && node.body.type === 'TSModuleBlock') {
          const block = newScope('block', scope, node.body.start, node.body.end);
          visitStatements(node.body.body, block);
        }
        return;
      }

      case 'TSImportEqualsDeclaration': {
        if (node.importKind === 'type') return;
        declare(scope, node.id, { kind: 'const', cat: 'LEX', declStart: node.start, declEnd: node.end });
        if (node.isExport) exportPending.push({ local: node.id.name, exported: node.id.name });
        return;
      }

      case 'TSExportAssignment':
        visit(node.expression, scope);
        return;

      case 'TSAsExpression':
      case 'TSSatisfiesExpression':
      case 'TSNonNullExpression':
      case 'TSTypeAssertion':
      case 'TSInstantiationExpression':
        visit(node.expression, scope);
        return;

      case 'JSXElement':
        for (const attr of node.openingElement.attributes) {
          if (attr.type === 'JSXAttribute' && attr.value && attr.value.type === 'JSXExpressionContainer') {
            visit(attr.value.expression, scope);
          } else if (attr.type === 'JSXSpreadAttribute') {
            visit(attr.argument, scope);
          }
        }
        for (const child of node.children) visit(child, scope);
        return;

      case 'JSXFragment':
        for (const child of node.children) visit(child, scope);
        return;

      case 'JSXExpressionContainer':
        if (node.expression.type !== 'JSXEmptyExpression') visit(node.expression, scope);
        return;

      case 'JSXSpreadChild':
        visit(node.expression, scope);
        return;

      case 'JSXText':
      case 'JSXEmptyExpression':
        return;

      default:
        // Unknown node: visit child nodes in source order, treating identifiers as reads.
        for (const key of Object.keys(node)) {
          if (SKIP_CHILD_KEYS.has(key)) continue;
          const value = node[key];
          if (Array.isArray(value)) {
            for (const v of value) if (isNode(v)) visit(v, scope);
          } else if (isNode(value)) {
            visit(value, scope);
          }
        }
    }
  }

  function visitFunctionDeclaration(node, scope, { exportName }) {
    if (node.declare) return;
    if (node.id) {
      const cat = ['function', 'function-body'].includes(scope.kind) ? 'VAR' : 'LEX';
      declare(scope, node.id, { kind: 'function', cat, initEnd: -1, declStart: node.start, declEnd: node.end });
      if (exportName) exportPending.push({ local: node.id.name, exported: exportName });
    }
    visitFunction(node, scope, { kind: 'declaration', hoisted: true, creationStart: scope.start });
  }

  // Duplicate and conflicting declarations, checked per scope once every declaration is known.
  function checkConflicts(s) {
    const groups = new Map();
    const push = (name, item) => {
      if (!groups.has(name)) groups.set(name, []);
      groups.get(name).push(item);
    };
    for (const d of s.decls) push(d.name, d);
    for (const t of s.through) push(t.name, { cat: 'VAR', start: t.start, end: t.end, name: t.name });
    for (const [name, items] of groups) {
      if (items.length < 2) continue;
      items.sort((a, b) => a.start - b.start);
      const prior = [];
      for (const d of items) {
        const conflict = prior.length > 0 && (
          d.cat === 'LEX' ||
          prior.some((p) => p.cat === 'LEX') ||
          (d.cat === 'PARAM' && prior.some((p) => p.cat === 'PARAM'))
        );
        if (conflict) addDiag('VN5001', d.start, d.end, `Identifier '${name}' has already been declared`);
        prior.push(d);
      }
    }
  }

  function ensureImplicitArguments(s) {
    const existing = s.names.get('arguments');
    if (existing) return existing;
    const fn = fns[s.fnId];
    const b = {
      id: bindings.length,
      name: 'arguments',
      kind: 'implicit',
      scopeId: s.id,
      start: fn.start,
      end: fn.start,
      declStart: fn.start,
      declEnd: fn.start,
      captured: false,
      mutated: false,
      needsTdzCheck: false,
      perIterationCopy: false,
      exported: [],
      initEnd: -1,
      forKind: null,
      inSwitch: false,
      selfName: false,
      importedName: null,
      source: null,
    };
    bindings.push(b);
    s.names.set('arguments', b);
    s.bindingIds.push(b.id);
    fn.usesArguments = true;
    return b;
  }

  function needsTdzForRef(b, r, crossed, outerFn) {
    if (b.kind === 'import') return true;
    if (!TDZ_KINDS.has(b.kind)) return false;
    if (b.inSwitch) return true;
    if (r.start < b.initEnd) return true;
    if (crossed && outerFn && outerFn.creationStart < b.initEnd) return true;
    return false;
  }

  function resolveRef(r, globalsUsed) {
    let s = scopes[r.scopeId];
    let crossed = false;
    let outerFn = null;
    let found = null;
    while (s) {
      let b = s.names.get(r.name);
      if (!b && r.name === 'arguments' && s.kind === 'function' && s.fnId >= 0 && !fns[s.fnId].isArrow) {
        b = ensureImplicitArguments(s);
      }
      if (b) { found = b; break; }
      if (s.kind === 'function') {
        crossed = true;
        outerFn = fns[s.fnId];
      }
      s = s.parentId >= 0 ? scopes[s.parentId] : null;
    }
    r.crossesFunction = crossed;
    if (!found) {
      r.binding = -1;
      globalsUsed.add(r.name);
      return;
    }
    r.binding = found.id;
    if (crossed) found.captured = true;
    if (r.write) {
      found.mutated = true;
      if (found.kind === 'import') {
        addDiag('VN5003', r.start, r.end, `Cannot assign to imported binding '${r.name}'`, 'warning');
      } else if (found.selfName) {
        addDiag('VN5004', r.start, r.end, `Cannot assign to '${r.name}', the name of the enclosing function expression`, 'warning');
      } else if (found.kind === 'const') {
        addDiag('VN5002', r.start, r.end, `Cannot assign to const binding '${r.name}' after initialization`, 'warning');
      }
    }
    if (needsTdzForRef(found, r, crossed, outerFn)) found.needsTdzCheck = true;
  }

  // Pass 1: build scopes, declare bindings and record references.
  const moduleScope = newScope('module', null, program.start, program.end);
  visitStatements(program.body, moduleScope);

  // Pass 2: conflicts, reference resolution, exports and per-iteration copies.
  for (const s of scopes) checkConflicts(s);
  const globalsUsed = new Set();
  for (const r of refs) resolveRef(r, globalsUsed);
  for (const e of exportPending) {
    const b = moduleScope.names.get(e.local);
    if (b && !b.exported.includes(e.exported)) b.exported.push(e.exported);
  }
  for (const b of bindings) {
    if (!b.captured || !b.forKind) continue;
    if (b.forKind === 'c-style' && b.kind === 'let') b.perIterationCopy = true;
    if (b.forKind === 'in-of' && (b.kind === 'let' || b.kind === 'const')) b.perIterationCopy = true;
  }

  diags.sort((a, b) => a.start.offset - b.start.offset || (a.code < b.code ? -1 : a.code > b.code ? 1 : 0));

  // ScopeInfo objects carry only plain data (the working maps and declaration lists stay internal).
  const scopeInfo = scopes.map((s) => ({
    id: s.id,
    kind: s.kind,
    parentId: s.parentId,
    start: s.start,
    end: s.end,
    fnId: s.fnId,
    bindingIds: s.bindingIds,
    childIds: s.childIds,
  }));

  return {
    module: scopeInfo[moduleScope.id],
    scopes: scopeInfo,
    bindings,
    references: refs,
    functions: fns,
    globalsUsed: [...globalsUsed].sort(),
    moduleUsesThis,
    diagnostics: diags,
  };
}

// Pretty-printer for debugging and tests: one line per scope, binding and reference.
export function describeScopes(result) {
  const out = [];
  for (const s of result.scopes) {
    out.push(`scope ${s.id} ${s.kind} [${s.start},${s.end}) parent=${s.parentId} fn=${s.fnId}`);
    for (const bid of s.bindingIds) {
      const b = result.bindings[bid];
      out.push(
        `  ${b.kind} ${b.name} captured=${b.captured} mutated=${b.mutated} tdz=${b.needsTdzCheck}` +
        ` perIteration=${b.perIterationCopy} exported=[${b.exported.join(',')}]`,
      );
    }
  }
  for (const r of result.references) {
    const target = r.binding >= 0 ? `binding#${r.binding}` : 'global';
    const mode = [r.read ? 'read' : '', r.write ? 'write' : '', r.typeofOnly ? 'typeof' : ''].filter(Boolean).join('+');
    out.push(`ref ${r.name}@${r.start} ${mode} -> ${target}`);
  }
  out.push(`globals: ${result.globalsUsed.join(', ')}`);
  return out.join('\n');
}
