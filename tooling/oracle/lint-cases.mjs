// Lint for language scenario cases (WP-O1). Checks the two header lines and rejects constructs outside the profile.
// (1) Forbidden syntax and names: a fixed list (classes, generators, regular expressions, Proxy/Reflect, Date, ...).
// (2) Builtins, derived from the standard-library manifest (compiler/profile/stdlib.json, read through
//     compiler/profile/stdlib.mjs): a free global, a static member chain on a builtin global and a builtin constructor
//     must be P or A for target 'test'. A method call on any other receiver must name a method that some builtin
//     prototype supports for 'test', a host timer method, or a member the case defines itself.
//     `trace` and `testHost` are the only host globals.
// Limits: receiver types are not inferred, so methods are checked by name; the engine-message rule covers try/catch
// clauses only (`.catch(...)` callbacks are reviewed by hand). Uses @babel/parser and a small AST walk.
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {readFile} from 'node:fs/promises';
import {parse} from '@babel/parser';
import {isStdlibSupported, listSupported, memberPath, resolveGlobal} from '../../compiler/profile/stdlib.mjs';
import {casesRoot} from './node-runner.mjs';
import {listCases} from './check-goldens.mjs';

const TARGET = 'test';
const HOST_GLOBALS = new Set(['trace', 'testHost']);
// Methods of testHost.timers, reached through aliases such as `const t = testHost.timers`.
const HOST_METHODS = new Set(['setTimeout', 'clearTimeout', 'now', 'advance', 'tick']);
const METHOD_NAME = /\.([A-Za-z_$][\w$]*)$/;
// Method names that some builtin prototype supports for the test target (receiver types are not inferred).
const SUPPORTED_METHODS = new Set(
  listSupported('proto')
    .filter((e) => isStdlibSupported(e.path, TARGET))
    .map((e) => METHOD_NAME.exec(e.path))
    .filter((m) => m !== null)
    .map((m) => m[1]),
);

const SKIP_KEYS = new Set([
  'loc', 'start', 'end', 'extra', 'comments', 'leadingComments', 'trailingComments', 'innerComments', 'tokens', 'errors',
]);
const FUNCTION_TYPES = new Set([
  'FunctionDeclaration', 'FunctionExpression', 'ArrowFunctionExpression', 'ObjectMethod', 'ClassMethod', 'ClassPrivateMethod',
]);
const PROPERTY_LIKE = new Set([
  'ObjectProperty', 'ObjectMethod', 'ClassProperty', 'ClassMethod', 'ClassPrivateProperty', 'ClassPrivateMethod',
]);
const INTL = new Set(['toLocaleString', 'toLocaleUpperCase', 'toLocaleLowerCase', 'localeCompare', 'toLocaleDateString', 'toLocaleTimeString']);
const FORBIDDEN_NAMES = new Set([
  'Proxy', 'Reflect', 'eval', 'Function', 'Date', 'Intl', 'BigInt', 'RegExp', 'arguments', '__proto__',
  'Int8Array', 'Uint8Array', 'Uint8ClampedArray', 'Int16Array', 'Uint16Array', 'Int32Array', 'Uint32Array',
  'Float32Array', 'Float64Array', 'BigInt64Array', 'BigUint64Array', 'ArrayBuffer', 'SharedArrayBuffer', 'DataView', 'Atomics',
  'WeakMap', 'WeakSet', 'WeakRef', 'FinalizationRegistry',
  'globalThis', 'global', 'process', 'require', 'module', 'exports', '__dirname', '__filename', 'Buffer', 'window', 'document', 'console',
  'setTimeout', 'clearTimeout', 'setInterval', 'clearInterval', 'setImmediate', 'clearImmediate', 'queueMicrotask',
]);
const TIER = /^\/\/ @tier [123]$/;
const COVERS = /^\/\/ @covers L(?:0[1-9]|1\d|2[01])(?:,L(?:0[1-9]|1\d|2[01]))*$/;

function* childrenOf(node) {
  for (const key of Object.keys(node)) {
    if (SKIP_KEYS.has(key)) continue;
    const v = node[key];
    if (Array.isArray(v)) {
      for (const c of v) if (c && typeof c.type === 'string') yield [key, c];
    } else if (v && typeof v.type === 'string') {
      yield [key, v];
    }
  }
}

// Calls fn(node, parent, key) for every node under root, root included.
function walk(node, fn, parent = null, key = null) {
  fn(node, parent, key);
  for (const [k, c] of childrenOf(node)) walk(c, fn, node, k);
}

const keyName = (k) => (k.type === 'Identifier' ? k.name : k.type === 'StringLiteral' ? k.value : null);

function memberName(m) {
  if (!m.computed && m.property.type === 'Identifier') return m.property.name;
  if (m.property.type === 'StringLiteral') return m.property.value;
  return null;
}

// True when an Identifier at (parent, key) is a variable reference, not a property name, label or import key.
function identifierIsReference(parent, key) {
  if (!parent) return true;
  if (parent.type === 'MemberExpression' || parent.type === 'OptionalMemberExpression') return key !== 'property' || parent.computed;
  if (PROPERTY_LIKE.has(parent.type)) return key !== 'key' || parent.computed;
  if (parent.type === 'LabeledStatement' || parent.type === 'BreakStatement' || parent.type === 'ContinueStatement') return key !== 'label';
  if (parent.type === 'ImportSpecifier') return key !== 'imported';
  return true;
}

// The identifier at the root of a member chain (a.b.c gives a), or null when the chain starts with something else.
function chainRoot(node) {
  let n = node;
  while (n && (n.type === 'MemberExpression' || n.type === 'OptionalMemberExpression')) n = n.object;
  return n && n.type === 'Identifier' ? n : null;
}

// Binding identifiers of a pattern.
function bindingNames(pattern, out) {
  if (!pattern) return;
  switch (pattern.type) {
    case 'Identifier':
      out.add(pattern.name);
      break;
    case 'AssignmentPattern':
      bindingNames(pattern.left, out);
      break;
    case 'RestElement':
      bindingNames(pattern.argument, out);
      break;
    case 'ArrayPattern':
      for (const el of pattern.elements) bindingNames(el, out);
      break;
    case 'ObjectPattern':
      for (const p of pattern.properties) bindingNames(p.type === 'RestElement' ? p.argument : p.value, out);
      break;
    default:
      break;
  }
}

// Names the case binds itself, and property names it defines (object keys, methods, assigned members).
function collectScope(program) {
  const declared = new Set();
  const props = new Set();
  walk(program, (n) => {
    switch (n.type) {
      case 'VariableDeclarator':
        bindingNames(n.id, declared);
        break;
      case 'FunctionDeclaration':
      case 'FunctionExpression':
      case 'ArrowFunctionExpression':
      case 'ObjectMethod':
      case 'ClassMethod':
        if (n.id) declared.add(n.id.name);
        for (const p of n.params) bindingNames(p, declared);
        if (n.type === 'ObjectMethod' && !n.computed && keyName(n.key) !== null) props.add(keyName(n.key));
        break;
      case 'ClassDeclaration':
      case 'ClassExpression':
        if (n.id) declared.add(n.id.name);
        break;
      case 'CatchClause':
        bindingNames(n.param, declared);
        break;
      case 'ImportSpecifier':
      case 'ImportDefaultSpecifier':
      case 'ImportNamespaceSpecifier':
        declared.add(n.local.name);
        break;
      case 'ObjectProperty':
        if (!n.computed && keyName(n.key) !== null) props.add(keyName(n.key));
        break;
      case 'AssignmentExpression':
        if (n.left.type === 'MemberExpression' && !n.left.computed && n.left.property.type === 'Identifier') {
          props.add(n.left.property.name);
        }
        break;
      default:
        break;
    }
  });
  return {declared, props};
}

// Forbidden member names: __proto__, Intl-dependent methods, and Math.random (nondeterministic).
function checkMember(node, add) {
  const name = memberName(node);
  if (name === '__proto__') add('PROTO', '__proto__ is outside the profile');
  if (name !== null && INTL.has(name)) add('INTL', `${name} depends on Intl/ICU and is outside the profile`);
  if (node.object.type === 'Identifier' && node.object.name === 'Math' && name === 'random') {
    add('RANDOM', 'Math.random is outside the profile');
  }
}

// A member chain rooted at a builtin global (Object.keys, Math.PI) must be supported for the test target. A computed
// or optional access on a builtin cannot be checked, so it is rejected. Only the outermost chain is reported.
function checkBuiltinMember(node, parent, key, st, add) {
  if (parent && (parent.type === 'MemberExpression' || parent.type === 'OptionalMemberExpression') && key === 'object') return;
  const root = chainRoot(node);
  if (root === null || st.declared.has(root.name) || HOST_GLOBALS.has(root.name)) return;
  if (resolveGlobal(root.name, TARGET) === null) return; // not a builtin: the identifier check reports it
  const dotted = memberPath(node);
  if (dotted === null) {
    add('UNLISTED_API', `access on builtin ${root.name} must be a plain dotted name to be checked`);
  } else if (!isStdlibSupported(dotted, TARGET)) {
    add('UNLISTED_API', `${dotted} is not a supported builtin for target ${TARGET} (stdlib manifest)`);
  }
}

// A free identifier must be a builtin that the manifest supports for the test target, or a host global.
function checkFreeIdentifier(node, st, add) {
  const name = node.name;
  if (st.declared.has(name) || HOST_GLOBALS.has(name) || isStdlibSupported(name, TARGET)) return;
  const g = resolveGlobal(name, TARGET);
  add('UNLISTED_API', g !== null && g.hint ? g.hint : `${name} is not a supported global for target ${TARGET}`);
}

// A method call whose receiver is not a builtin global or a host object. Receiver types are not inferred, so the
// method name must be supported by some builtin prototype, be a host timer method, or be defined by the case itself.
function checkMethodCall(node, st, add) {
  const callee = node.callee;
  if (callee.type !== 'MemberExpression' && callee.type !== 'OptionalMemberExpression') return;
  const root = chainRoot(callee.object);
  if (root !== null) {
    if (HOST_GLOBALS.has(root.name)) return;
    if (!st.declared.has(root.name) && resolveGlobal(root.name, TARGET) !== null) return; // checked by checkBuiltinMember
  }
  const name = memberName(callee);
  if (name === null) {
    add('UNLISTED_API', 'computed method call cannot be checked against the stdlib manifest');
  } else if (!SUPPORTED_METHODS.has(name) && !HOST_METHODS.has(name) && !st.props.has(name)) {
    add('UNLISTED_API', `.${name}() is not a supported builtin method and is not defined by the case`);
  }
}

// True when an argument of trace() may record the catch parameter itself or an engine error message.
// Engine errors carry name, message and stack; the bare value, message/stack/toString/valueOf reads and computed
// reads can expose them. Allowed: other property reads (e.name, e.reason), identity and typeof, `e instanceof X`.
const BLOCKED_MEMBERS = new Set(['message', 'stack', 'toString', 'valueOf']);
function leaksError(arg, name) {
  let leak = false;
  walk(arg, (n, p, k) => {
    if (n.type !== 'Identifier' || n.name !== name) return;
    if (!identifierIsReference(p, k)) return;
    if (p && (p.type === 'MemberExpression' || p.type === 'OptionalMemberExpression') && k === 'object') {
      const prop = memberName(p);
      if (prop === null || BLOCKED_MEMBERS.has(prop)) leak = true;
      return;
    }
    if (p && p.type === 'BinaryExpression' && ['===', '!==', 'instanceof'].includes(p.operator)) {
      if (p.operator !== 'instanceof' || k === 'left') return;
    }
    if (p && p.type === 'UnaryExpression' && p.operator === 'typeof') return;
    leak = true;
  });
  return leak;
}

function checkCatchMessages(tryNode, st) {
  const h = tryNode.handler;
  if (!h || !h.param || h.param.type !== 'Identifier') return;
  let throwsOwn = false;
  walk(tryNode.block, (n) => {
    if (n.type === 'ThrowStatement') throwsOwn = true;
  });
  if (throwsOwn) return; // this try block throws scenario-created values; their messages are allowed
  const name = h.param.name;
  walk(h.body, (n) => {
    if (n.type !== 'CallExpression' || n.callee.type !== 'Identifier' || n.callee.name !== 'trace') return;
    for (const arg of n.arguments) {
      if (leaksError(arg, name)) {
        st.problems.push({
          code: 'ENGINE_MESSAGE',
          line: n.loc ? n.loc.start.line : 0,
          message: `trace() of catch parameter "${name}" may record an engine error message; trace ${name}.name or ${name} instanceof X`,
        });
        return;
      }
    }
  });
}

function checkNode(node, parent, key, st) {
  const add = (code, message) => st.problems.push({code, line: node.loc ? node.loc.start.line : 0, message});
  switch (node.type) {
    case 'ClassDeclaration':
    case 'ClassExpression':
      add('CLASS', 'class syntax is outside the profile');
      break;
    case 'NewExpression': {
      const c = node.callee;
      const builtin = c.type === 'Identifier' && !st.declared.has(c.name) && isStdlibSupported(c.name, TARGET);
      if (!builtin) add('NEW', '`new` is only allowed for builtin constructors that the stdlib manifest supports for target test');
      break;
    }
    case 'RegExpLiteral':
      add('REGEXP', 'regular expression literals are outside the profile');
      break;
    case 'BigIntLiteral':
      add('BIGINT', 'BigInt literals are outside the profile');
      break;
    case 'WithStatement':
      add('WITH', '`with` is outside the profile');
      break;
    case 'YieldExpression':
      add('GENERATOR', 'generators are outside the profile');
      break;
    case 'AwaitExpression':
      if (st.fnDepth === 0) add('TOP_LEVEL_AWAIT', 'top-level await is outside the profile');
      break;
    case 'ForOfStatement':
      if (node.await) add('FOR_AWAIT', 'for await is outside the profile');
      break;
    case 'Super':
      add('SUPER', '`super` is outside the profile');
      break;
    case 'MetaProperty':
      add('META', 'import.meta and new.target are outside the profile');
      break;
    case 'Import':
      add('DYNAMIC_IMPORT', 'dynamic import is outside the profile');
      break;
    case 'ImportDeclaration':
      if (node.source.value !== 'vue') add('IMPORT', "only imports from 'vue' are allowed");
      break;
    case 'ExportDefaultDeclaration':
    case 'ExportAllDeclaration':
      add('EXPORT', 'only `export function run` / `export async function run` are allowed');
      break;
    case 'ExportNamedDeclaration': {
      const d = node.declaration;
      if (node.specifiers.length > 0 || !d || d.type !== 'FunctionDeclaration' || !d.id || d.id.name !== 'run') {
        add('EXPORT', 'only `export function run` / `export async function run` are allowed');
      }
      break;
    }
    case 'LabeledStatement':
      if (node.body.type === 'FunctionDeclaration') add('LABELED_FUNCTION', 'labeled function declarations are outside the profile');
      break;
    case 'FunctionDeclaration':
    case 'FunctionExpression':
    case 'ObjectMethod':
    case 'ClassMethod':
      if (node.generator) add('GENERATOR', 'generators are outside the profile');
      break;
    case 'ObjectProperty':
      if (!node.computed && keyName(node.key) === '__proto__') add('PROTO', '__proto__ is outside the profile');
      break;
    case 'MemberExpression':
    case 'OptionalMemberExpression':
      checkMember(node, add);
      checkBuiltinMember(node, parent, key, st, add);
      break;
    case 'CallExpression':
    case 'OptionalCallExpression':
      checkMethodCall(node, st, add);
      break;
    case 'Identifier':
      if (!identifierIsReference(parent, key)) break;
      if (FORBIDDEN_NAMES.has(node.name)) add('FORBIDDEN_NAME', `\`${node.name}\` is outside the profile`);
      else checkFreeIdentifier(node, st, add);
      break;
    case 'TryStatement':
      checkCatchMessages(node, st);
      break;
    default:
      break;
  }
}

function visit(node, parent, key, st) {
  const isFn = FUNCTION_TYPES.has(node.type);
  if (isFn) st.fnDepth++;
  checkNode(node, parent, key, st);
  for (const [k, c] of childrenOf(node)) visit(c, node, k, st);
  if (isFn) st.fnDepth--;
}

// Header: exactly `// @tier N` then `// @covers L01,L02` as the first two lines.
export function checkHeader(source) {
  const lines = source.split('\n');
  const problems = [];
  if (!TIER.test((lines[0] ?? '').trimEnd())) {
    problems.push({code: 'HEADER', line: 1, message: 'first line must be "// @tier N" with N = 1, 2 or 3'});
  }
  if (!COVERS.test((lines[1] ?? '').trimEnd())) {
    problems.push({code: 'HEADER', line: 2, message: 'second line must be "// @covers L01,L02" with IDs L01..L21'});
  }
  return problems;
}

// Returns [{code, line, message}] for one source text; empty when the case is clean.
export function lintSource(source) {
  const problems = checkHeader(source);
  let ast;
  try {
    ast = parse(source, {sourceType: 'module'});
  } catch (e) {
    problems.push({code: 'PARSE', line: e.loc ? e.loc.line : 0, message: e.message});
    return problems;
  }
  const {declared, props} = collectScope(ast.program);
  visit(ast.program, null, null, {fnDepth: 0, problems, declared, props});
  return problems;
}

export async function lintFile(file) {
  return lintSource(await readFile(file, 'utf8'));
}

const isMain = process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  const files = await listCases(casesRoot);
  let count = 0;
  for (const file of files) {
    for (const p of await lintFile(file)) {
      count += 1;
      process.stdout.write(`${path.relative(casesRoot, file)}:${p.line} [${p.code}] ${p.message}\n`);
    }
  }
  process.stdout.write(`${files.length} case(s), ${count} problem(s)\n`);
  process.exitCode = count === 0 ? 0 : 1;
}
