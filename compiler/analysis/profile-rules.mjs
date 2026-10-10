// Language profile rules as data (design.md section 8; manifest rows L01-L21, S01-S25).
// Each rule: { id, code, row, langs, targets?, kind, appliesTo, span, message, hint, ... }
//   kind 'node'  : match(node, ctx, ancestors) on AST nodes of nodeTypes; span(node, ctx) -> [start, end)
//   kind 'ref'   : match(ref, ctx) on scope references (globals, implicit arguments); span is the reference
//   kind 'parse' : reasons = @babel/parser reasonCode values that module parsing reports for sloppy-only or
//                  duplicate-declaration forms; the parser maps them to this rule (see compiler/frontend/parse.mjs)
// message / hint are the diagnostic text. The code is registered in compiler/diagnostics/codes.mjs.

const KNOWN_CONSTRUCTORS = new Set([
  'Map', 'Set', 'Promise', 'Error', 'TypeError', 'RangeError', 'SyntaxError', 'ReferenceError', 'URIError',
  'EvalError', 'AggregateError', 'Array', 'Object', 'String', 'Number', 'Boolean', 'ArrayBuffer', 'Uint8Array',
]);

const FUNCTION_TYPES = new Set([
  'FunctionDeclaration', 'FunctionExpression', 'ArrowFunctionExpression', 'ObjectMethod', 'ClassMethod',
  'ClassPrivateMethod',
]);

const STATEMENT_POSITION_PARENTS = new Set([
  'IfStatement', 'LabeledStatement', 'WhileStatement', 'DoWhileStatement', 'ForStatement', 'ForInStatement',
  'ForOfStatement', 'WithStatement',
]);

const CLASS_TYPES = ['ClassDeclaration', 'ClassExpression'];

// Language modes: js and ts are plain modules; jsx and tsx add JSX. Type syntax is accepted in ts and tsx only.
const ALL_LANGS = Object.freeze(['js', 'ts', 'jsx', 'tsx']);
const JS_FAMILY = Object.freeze(['js', 'jsx']);
const TS_FAMILY = Object.freeze(['ts', 'tsx']);

function isIdent(node, name) {
  return !!node && node.type === 'Identifier' && (name === undefined || node.name === name);
}

function literalKey(node) {
  if (!node) return null;
  if (node.type === 'Identifier') return node.name;
  if (node.type === 'StringLiteral') return node.value;
  return null;
}

// Span of a keyword that begins at node.start (the first occurrence of word at or after node.start).
function keywordSpan(ctx, node, word) {
  const at = ctx.source.indexOf(word, node.start);
  if (at < 0 || at >= node.end) return [node.start, Math.min(node.end, node.start + word.length)];
  return [at, at + word.length];
}

// Span from the start of a node up to its body, without trailing whitespace (a function or class head).
function headSpan(node, ctx) {
  const bodyStart = node.body ? node.body.start : node.end;
  let end = bodyStart;
  while (end > node.start && /\s/.test(ctx.source[end - 1])) end--;
  return [node.start, end];
}

function nodeSpan(node) {
  return [node.start, node.end];
}

// A reference resolves to a global when no declaration in the module binds its name.
function refFor(node, ctx) {
  return ctx.refByStart.get(node.start) || null;
}

function isGlobalIdentifier(node, ctx) {
  if (!isIdent(node)) return false;
  const ref = refFor(node, ctx);
  return !!ref && ref.binding === -1;
}

function isImportBinding(node, ctx) {
  const ref = refFor(node, ctx);
  return !!ref && ref.binding >= 0 && ctx.analysis.bindings[ref.binding].kind === 'import';
}

function isSupportedConstructorCallee(callee, ctx) {
  if (!isIdent(callee)) return false;
  if (isGlobalIdentifier(callee, ctx)) return KNOWN_CONSTRUCTORS.has(callee.name);
  return isImportBinding(callee, ctx);
}

function isLiteralSpecifier(arg) {
  return !!arg && (arg.type === 'StringLiteral' || (arg.type === 'TemplateLiteral' && arg.expressions.length === 0));
}

function isFunctionNode(node) {
  return FUNCTION_TYPES.has(node.type);
}

const OBJECT_METAPROGRAMMING = new Set(['setPrototypeOf', 'defineProperty', 'defineProperties']);

const RULES = [
  // ---- parse-time mappings: sloppy-only and duplicate forms that module parsing rejects ----
  {
    id: 'with', code: 'VN2001', row: 'L01', langs: ALL_LANGS, kind: 'node', nodeTypes: ['WithStatement'],
    reasons: ['StrictWith'],
    appliesTo: () => true,
    span: (node, ctx) => keywordSpan(ctx, node, 'with'),
    message: "'with' statement is not supported",
    hint: 'Read the properties you need explicitly, or bind the object to a const and use its properties.',
  },
  {
    id: 'annex-b-function', code: 'VN2023', row: 'L01', langs: ALL_LANGS, kind: 'node',
    nodeTypes: ['FunctionDeclaration'], reasons: ['StrictFunction'],
    appliesTo: (node, ctx, ancestors) => STATEMENT_POSITION_PARENTS.has(ancestors[ancestors.length - 1]?.type),
    span: (node, ctx) => keywordSpan(ctx, node, 'function'),
    message: 'Function declaration in a statement position is not supported',
    hint: 'Wrap the function declaration in a block, or bind a function expression to a const.',
  },
  {
    id: 'legacy-octal', code: 'VN2024', row: 'L08', langs: ALL_LANGS, kind: 'node', nodeTypes: ['NumericLiteral'],
    reasons: ['StrictOctalLiteral'],
    appliesTo: (node) => /^0[0-9]/.test(node.extra?.raw ?? ''),
    span: nodeSpan,
    message: 'Legacy octal literal is not supported',
    hint: 'Write the value in decimal, or use the 0o prefix.',
  },
  {
    id: 'legacy-octal-escape', code: 'VN2025', row: 'L09', langs: ALL_LANGS, kind: 'parse',
    reasons: ['StrictNumericEscape'],
    message: 'Legacy octal escape sequence is not supported in strings',
    hint: 'Write the code unit with a \\u or \\x escape.',
  },
  {
    id: 'delete-identifier', code: 'VN2018', row: 'L04', langs: ALL_LANGS, kind: 'node', nodeTypes: ['UnaryExpression'],
    reasons: ['StrictDelete'],
    appliesTo: (node) => node.operator === 'delete' && isIdent(node.argument),
    span: (node, ctx) => keywordSpan(ctx, node, 'delete'),
    message: 'Deleting a variable name is not supported',
    hint: 'Delete a property of an object (delete obj.key), or assign undefined.',
  },
  {
    id: 'eval-assignment', code: 'VN2026', row: 'L20', langs: ALL_LANGS, kind: 'node', nodeTypes: ['AssignmentExpression'],
    reasons: ['StrictEvalArguments'],
    appliesTo: (node) => isIdent(node.left) && (node.left.name === 'eval' || node.left.name === 'arguments'),
    span: (node) => [node.left.start, node.left.end],
    message: 'Assignment to eval or arguments is not supported',
    hint: 'Use a differently named variable.',
  },
  {
    id: 'arguments-binding', code: 'VN2027', row: 'L02', langs: ALL_LANGS, kind: 'parse',
    reasons: ['StrictEvalArgumentsBinding'],
    message: 'A binding named eval or arguments is not supported',
    hint: 'Rename the binding.',
  },
  {
    id: 'duplicate-declaration', code: 'VN5001', row: 'L01', langs: ALL_LANGS, kind: 'parse',
    reasons: ['VarRedeclaration', 'ParamDupe'], useBabelMessage: true,
    message: 'Duplicate declaration',
    hint: 'Rename or remove one of the declarations.',
  },

  // ---- JavaScript profile rules over AST nodes ----
  {
    id: 'class', code: 'VN2002', row: 'L05', langs: ALL_LANGS, kind: 'node', nodeTypes: CLASS_TYPES,
    appliesTo: (node) => !node.declare,
    span: headSpan,
    message: 'Classes are not supported',
    hint: 'Use a plain object, a factory function or a composable function instead of a class.',
  },
  {
    id: 'class-extends', code: 'VN2003', row: 'L05', langs: ALL_LANGS, kind: 'node', nodeTypes: CLASS_TYPES,
    appliesTo: (node) => !node.declare && !!node.superClass,
    span: (node) => [node.superClass.start, node.superClass.end],
    message: 'Class inheritance (extends) is not supported',
    hint: 'Compose behaviour with functions or plain objects instead of inheritance.',
  },
  {
    id: 'super', code: 'VN2004', row: 'L05', langs: ALL_LANGS, kind: 'node', nodeTypes: ['Super'],
    appliesTo: () => true,
    span: nodeSpan,
    message: 'super is not supported',
    hint: 'Call the shared function directly instead of using super.',
  },
  {
    id: 'private-name', code: 'VN2005', row: 'L05', langs: ALL_LANGS, kind: 'node', nodeTypes: ['PrivateName'],
    appliesTo: () => true,
    span: nodeSpan,
    message: 'Private class names are not supported',
    hint: 'Use a closure variable or an ordinary property instead.',
  },
  {
    id: 'static-block', code: 'VN2006', row: 'L05', langs: ALL_LANGS, kind: 'node', nodeTypes: ['StaticBlock'],
    appliesTo: () => true,
    span: (node, ctx) => keywordSpan(ctx, node, 'static'),
    message: 'Static initialization blocks are not supported',
    hint: 'Move the initialization into a module-level const or function.',
  },
  {
    id: 'new-unknown', code: 'VN2007', row: 'L05', langs: ALL_LANGS, kind: 'node', nodeTypes: ['NewExpression'],
    appliesTo: (node, ctx) => !isSupportedConstructorCallee(node.callee, ctx),
    span: (node) => [node.callee.start, node.callee.end],
    message: '`new` is supported only for built-in constructors and imported bindings',
    hint: 'Use a factory function, or import the constructor from a supported module.',
  },
  {
    id: 'generator', code: 'VN2008', row: 'L15', langs: ALL_LANGS, kind: 'node',
    nodeTypes: ['FunctionDeclaration', 'FunctionExpression', 'ObjectMethod', 'ClassMethod', 'ClassPrivateMethod'],
    appliesTo: (node) => !!node.generator,
    span: headSpan,
    message: 'Generator and async generator functions are not supported',
    hint: 'Use an async function, or return the values as an array.',
  },
  {
    id: 'yield', code: 'VN2009', row: 'L15', langs: ALL_LANGS, kind: 'node', nodeTypes: ['YieldExpression'],
    appliesTo: () => true,
    span: (node, ctx) => keywordSpan(ctx, node, 'yield'),
    message: 'yield is not supported',
    hint: 'Return the values explicitly instead of yielding them.',
  },
  {
    id: 'for-await', code: 'VN2010', row: 'L15', langs: ALL_LANGS, kind: 'node', nodeTypes: ['ForOfStatement'],
    appliesTo: (node) => !!node.await,
    span: (node, ctx) => {
      const at = ctx.source.indexOf('await', node.start);
      return [node.start, at >= 0 && at < node.end ? at + 5 : node.start + 3];
    },
    message: 'for await loops are not supported',
    hint: 'Await each value explicitly inside an ordinary for...of loop.',
  },
  {
    id: 'regexp-literal', code: 'VN2011', row: 'L18', langs: ALL_LANGS, kind: 'node', nodeTypes: ['RegExpLiteral'],
    appliesTo: () => true,
    span: nodeSpan,
    message: 'Regular expression literals are not supported',
    hint: 'Use String methods such as indexOf, startsWith or split.',
  },
  {
    id: 'bigint-literal', code: 'VN2012', row: 'L08', langs: ALL_LANGS, kind: 'node', nodeTypes: ['BigIntLiteral'],
    appliesTo: () => true,
    span: nodeSpan,
    message: 'BigInt literals are not supported',
    hint: 'Use a Number with documented bounds.',
  },
  {
    id: 'tagged-template', code: 'VN2013', row: 'L09', langs: ALL_LANGS, kind: 'node',
    nodeTypes: ['TaggedTemplateExpression'],
    appliesTo: () => true,
    span: (node) => [node.tag.start, node.tag.end],
    message: 'Tagged template literals are not supported',
    hint: 'Call a function with the string as an ordinary argument.',
  },
  {
    id: 'debugger', code: 'VN2014', row: 'L01', langs: ALL_LANGS, kind: 'node', nodeTypes: ['DebuggerStatement'],
    appliesTo: () => true,
    span: (node, ctx) => keywordSpan(ctx, node, 'debugger'),
    message: 'debugger statements are not supported',
    hint: 'Remove the statement.',
  },
  {
    id: 'arguments', code: 'VN2015', row: 'L02', langs: ALL_LANGS, kind: 'ref',
    appliesTo: (ref, ctx) => ref.name === 'arguments' &&
      (ref.binding === -1 || ctx.analysis.bindings[ref.binding].kind === 'implicit'),
    span: (ref) => [ref.start, ref.end],
    message: 'The arguments object is not supported',
    hint: 'Use a rest parameter (...args).',
  },
  {
    id: 'proto-literal', code: 'VN2016', row: 'L05', langs: ALL_LANGS, kind: 'node', nodeTypes: ['ObjectProperty'],
    appliesTo: (node) => !node.computed && !node.shorthand && literalKey(node.key) === '__proto__',
    span: (node) => [node.key.start, node.key.end],
    message: 'Setting __proto__ in an object literal is not supported',
    hint: 'Use an ordinary property with a different key.',
  },
  {
    id: 'proto-access', code: 'VN2017', row: 'L05', langs: ALL_LANGS, kind: 'node',
    nodeTypes: ['MemberExpression', 'OptionalMemberExpression'],
    appliesTo: (node) => !node.computed && isIdent(node.property, '__proto__'),
    span: (node) => [node.property.start, node.property.end],
    message: 'Access to __proto__ is not supported',
    hint: 'Use ordinary properties; prototype replacement is not supported.',
  },
  {
    id: 'dynamic-import', code: 'VN2019', row: 'L17', langs: ALL_LANGS, kind: 'node',
    nodeTypes: ['CallExpression', 'OptionalCallExpression'],
    appliesTo: (node) => node.callee.type === 'Import' && !isLiteralSpecifier(node.arguments[0]),
    span: (node) => (node.arguments[0] ? [node.arguments[0].start, node.arguments[0].end] : nodeSpan(node)),
    message: 'import() is supported only with a literal module specifier',
    hint: 'Use a string literal, for example import("./module.js").',
  },
  {
    id: 'dynamic-import-options', code: 'VN2031', row: 'L17', langs: ALL_LANGS, kind: 'node',
    nodeTypes: ['CallExpression', 'OptionalCallExpression'],
    appliesTo: (node) => node.callee.type === 'Import' && node.arguments.length > 1,
    span: (node) => [node.arguments[1].start, node.arguments[node.arguments.length - 1].end],
    message: 'import() with import attributes or options is not supported',
    hint: 'Pass only the module specifier, for example import("./module.js").',
  },
  {
    id: 'import-meta', code: 'VN2020', row: 'L16', langs: ALL_LANGS, kind: 'node', nodeTypes: ['MetaProperty'],
    appliesTo: (node) => isIdent(node.meta, 'import'),
    span: nodeSpan,
    message: 'import.meta is not supported',
    hint: 'Pass the configuration explicitly through props or a service.',
  },
  {
    id: 'new-target', code: 'VN2021', row: 'L05', langs: ALL_LANGS, kind: 'node', nodeTypes: ['MetaProperty'],
    appliesTo: (node) => isIdent(node.meta, 'new'),
    span: nodeSpan,
    message: 'new.target is not supported',
    hint: 'Pass an explicit parameter or use a separate function.',
  },
  {
    id: 'top-level-await', code: 'VN2022', row: 'L16', langs: ALL_LANGS, kind: 'node', nodeTypes: ['AwaitExpression'],
    appliesTo: (node, ctx, ancestors) => !ancestors.some(isFunctionNode),
    span: (node, ctx) => keywordSpan(ctx, node, 'await'),
    message: 'Top-level await is not supported',
    hint: 'Move the await into an async function and call that function from setup.',
  },
  {
    id: 'jsx-js', code: 'VN2028', row: 'S21', langs: ['jsx'], kind: 'node', nodeTypes: ['JSXElement', 'JSXFragment'],
    appliesTo: () => true,
    span: nodeSpan,
    message: 'JSX is not supported',
    hint: 'use a <template> in a .vue file',
  },
  {
    id: 'decorators-js', code: 'VN2029', row: 'S21', langs: JS_FAMILY, kind: 'node', nodeTypes: ['Decorator'],
    appliesTo: () => true,
    span: nodeSpan,
    message: 'Decorators are not supported',
    hint: 'Use an explicit function call instead of a decorator.',
  },
  {
    id: 'accessor-js', code: 'VN2030', row: 'S21', langs: JS_FAMILY, kind: 'node', nodeTypes: ['ClassAccessorProperty'],
    appliesTo: () => true,
    span: nodeSpan,
    message: 'Auto-accessor class fields are not supported',
    hint: 'Use a ref or a computed value.',
  },
  {
    id: 'constructor-codegen', code: 'VN4012', row: 'L20', langs: ALL_LANGS, kind: 'node',
    nodeTypes: ['CallExpression', 'OptionalCallExpression'],
    appliesTo: (node) => node.callee.type === 'MemberExpression' && !node.callee.computed &&
      isIdent(node.callee.property, 'constructor'),
    span: (node) => [node.callee.start, node.callee.end],
    message: 'Calling a constructor property is not supported',
    hint: 'Write the logic directly.',
  },
  {
    id: 'object-metaprogramming', code: 'VN4008', row: 'L05', langs: ALL_LANGS, kind: 'node',
    nodeTypes: ['CallExpression', 'OptionalCallExpression'],
    appliesTo: (node, ctx) => node.callee.type === 'MemberExpression' && !node.callee.computed &&
      isIdent(node.callee.object, 'Object') && isGlobalIdentifier(node.callee.object, ctx) &&
      node.callee.property.type === 'Identifier' && OBJECT_METAPROGRAMMING.has(node.callee.property.name),
    span: (node) => [node.callee.start, node.callee.end],
    message: 'Object.defineProperty, Object.defineProperties and Object.setPrototypeOf are not supported',
    hint: 'Use plain object literals or explicit properties.',
  },
  {
    id: 'symbol-dispose', code: 'VN4015', row: 'S24', langs: ALL_LANGS, kind: 'node',
    nodeTypes: ['MemberExpression', 'OptionalMemberExpression'],
    appliesTo: (node, ctx) => isIdent(node.object, 'Symbol') && isGlobalIdentifier(node.object, ctx) &&
      !node.computed && (isIdent(node.property, 'dispose') || isIdent(node.property, 'asyncDispose')),
    span: (node) => [node.property.start, node.property.end],
    message: 'Symbol.dispose and Symbol.asyncDispose are not supported',
    hint: 'Use try/finally with an explicit cleanup call.',
  },

  // ---- global names: references that resolve to no module binding ----
  globalRule('global-eval', 'VN4001', 'L20', ['eval'], 'eval is not supported', 'Write the logic directly as code.'),
  globalRule('global-function', 'VN4002', 'L20', ['Function'], 'Function constructor is not supported', 'Write the function directly.'),
  globalRule('global-proxy', 'VN4003', 'L20', ['Proxy'], 'Proxy is not supported', 'Use ref, reactive or computed from the Vue API.'),
  globalRule('global-reflect', 'VN4004', 'L20', ['Reflect'], 'Reflect is not supported', 'Use the direct operation, such as property access or Object.keys.'),
  globalRule('global-bigint', 'VN4005', 'L08', ['BigInt'], 'BigInt is not supported', 'Use a Number with documented bounds.'),
  globalRule('global-regexp', 'VN4006', 'L18', ['RegExp'], 'RegExp is not supported', 'Use String methods or a host validation service.'),
  globalRule('global-weak', 'VN4007', 'L11', ['WeakMap', 'WeakSet', 'WeakRef', 'FinalizationRegistry'], 'Weak collections and finalizers are not supported', 'Use Map or Set and remove entries explicitly.'),
  globalRule('global-commonjs', 'VN4009', 'L16', ['require', 'module', 'exports'], 'CommonJS globals are not supported', 'Use import and export.'),
  globalRule('global-node', 'VN4010', 'S23', ['process', 'Buffer', 'global', '__dirname', '__filename'], 'Node.js globals are not supported', 'Use the @vue-native platform services.'),
  globalRule('global-dom', 'VN4011', 'S23', ['document', 'window', 'navigator', 'localStorage', 'sessionStorage', 'location', 'XMLHttpRequest', 'WebSocket'], 'Browser DOM globals are not available in the native target', 'Use the @vue-native platform services, or build for the web target.', ['native']),
  globalRule('global-this', 'VN4013', 'L20', ['globalThis'], 'globalThis is not supported', 'Import the value you need.'),
  globalRule('global-typed-arrays', 'VN4014', 'L18', ['Int8Array', 'Uint8ClampedArray', 'Int16Array', 'Uint16Array', 'Int32Array', 'Uint32Array', 'Float32Array', 'Float64Array', 'BigInt64Array', 'BigUint64Array', 'DataView'], 'Typed arrays other than Uint8Array and DataView are not supported', 'Use Uint8Array and ArrayBuffer with explicit bounds.'),

  // ---- TypeScript constructs that emit runtime code (types are erased and accepted) ----
  {
    id: 'ts-enum', code: 'VN3001', row: 'S20', langs: TS_FAMILY, kind: 'node', nodeTypes: ['TSEnumDeclaration'],
    appliesTo: (node) => !node.declare,
    span: (node) => [node.start, node.id.end],
    message: 'TypeScript enums are not supported',
    hint: 'Use a union of string literal types with a const object or plain constants.',
  },
  {
    id: 'ts-namespace', code: 'VN3002', row: 'S20', langs: TS_FAMILY, kind: 'node', nodeTypes: ['TSModuleDeclaration'],
    appliesTo: (node) => !node.declare,
    span: (node) => [node.start, node.id.end],
    message: 'TypeScript namespaces and module blocks are not supported',
    hint: 'Use an ES module (import and export) instead of a namespace.',
  },
  {
    id: 'ts-import-equals', code: 'VN3003', row: 'S25', langs: TS_FAMILY, kind: 'node',
    nodeTypes: ['TSImportEqualsDeclaration'],
    appliesTo: (node) => node.importKind !== 'type',
    span: nodeSpan,
    message: 'import = alias is not supported',
    hint: 'Use import { name } from "module".',
  },
  {
    id: 'ts-export-equals', code: 'VN3004', row: 'S25', langs: TS_FAMILY, kind: 'node', nodeTypes: ['TSExportAssignment'],
    appliesTo: () => true,
    span: nodeSpan,
    message: 'export = is not supported',
    hint: 'Use named or default exports.',
  },
  {
    id: 'ts-decorator', code: 'VN3005', row: 'S21', langs: TS_FAMILY, kind: 'node', nodeTypes: ['Decorator'],
    appliesTo: () => true,
    span: nodeSpan,
    message: 'Decorators are not supported',
    hint: 'Use explicit function calls instead of a decorator.',
  },
  {
    id: 'ts-param-property', code: 'VN3006', row: 'S20', langs: TS_FAMILY, kind: 'node', nodeTypes: ['TSParameterProperty'],
    appliesTo: () => true,
    span: nodeSpan,
    message: 'Constructor parameter properties are not supported',
    hint: 'Declare the field and assign it explicitly.',
  },
  {
    id: 'ts-accessor', code: 'VN3007', row: 'S21', langs: TS_FAMILY, kind: 'node', nodeTypes: ['ClassAccessorProperty'],
    appliesTo: () => true,
    span: nodeSpan,
    message: 'Auto-accessor class fields are not supported',
    hint: 'Use a ref or a computed value.',
  },
  {
    id: 'ts-using', code: 'VN3008', row: 'S24', langs: TS_FAMILY, kind: 'node', nodeTypes: ['VariableDeclaration'],
    appliesTo: (node) => (node.kind === 'using' || node.kind === 'await using') && !node.declare,
    span: (node, ctx) => {
      // Keyword head only: "using" or "await using", without the space before the first binding.
      let end = node.declarations[0].start;
      while (end > node.start && /\s/.test(ctx.source[end - 1])) end--;
      return [node.start, end];
    },
    message: 'using and await using declarations are not supported',
    hint: 'Use try/finally with an explicit dispose call.',
  },
  {
    id: 'ts-jsx', code: 'VN3009', row: 'S21', langs: ['tsx'], kind: 'node', nodeTypes: ['JSXElement', 'JSXFragment'],
    appliesTo: () => true,
    span: nodeSpan,
    message: 'JSX and TSX are not supported',
    hint: 'use a <template> in a .vue file',
  },
];

export const PROFILE_RULES = Object.freeze(RULES.map((rule) => Object.freeze(rule)));

function globalRule(id, code, row, names, message, hint, targets) {
  const rule = {
    id, code, row, langs: ALL_LANGS, kind: 'ref', names,
    appliesTo: (ref) => ref.binding === -1 && names.includes(ref.name),
    span: (ref) => [ref.start, ref.end],
    message,
    hint,
  };
  if (targets) rule.targets = targets;
  return rule;
}

export function ruleById(id) {
  const rule = PROFILE_RULES.find((r) => r.id === id);
  if (!rule) throw new RangeError(`unknown profile rule: ${id}`);
  return rule;
}
