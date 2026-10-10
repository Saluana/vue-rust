// Imported types that a compiler macro reads (VN6017). The official compiler (compileScript) reads the types of
// defineProps, defineModel and defineEmits through the module graph, and some of those reads fail silently: a prop whose
// type cannot be read is emitted untyped, and only a top-level reference fails the build. So an import the compiler
// cannot read is rejected here, before compileScript runs, instead of being approximated:
//   - a name from a package or an absolute path is rejected statically: compileScript reads only relative project files
//     (TypeScript is not registered, so nothing else resolves). A type-only import of a missing file is VN6017 in
//     script.mjs, and a value import of one is reported by the project graph (VN9001);
//   - a namespace import (import * as N) is rejected statically: compileScript never reads a member of one, valid or not;
//   - a typeof of an imported value is rejected statically: compileScript does not read the type of a value here;
//   - a name from a relative file is probed: a one-macro component with the same import is compiled with the same
//     root-limited fs, and the failure at that component's own macro (not inside the imported file) means the file does
//     not provide the name as a type. This is the compiler's own name resolution, not a copy of TypeScript's.
// Which type arguments are read: defineProps and defineModel read every type reference of the type argument, including
// the ones inside prop types, and the local declarations they name. defineEmits reads only its emits type (a reference,
// or the union, intersection and extends of references); the payload types of events are never read. defineSlots and
// the other macros read no types, so they are not checked.
import { parse as parseSfc, compileScript } from '@vue/compiler-sfc';

// Macro name -> which of its type-argument nodes the compiler reads: 'all' or 'shape' (see above).
const TYPE_MACROS = new Map([['defineProps', 'all'], ['defineModel', 'all'], ['defineEmits', 'shape']]);

// Under 'shape', only these nodes lead to more references; a type literal, function type or array holds payloads.
const SHAPE_DESCENT = new Set([
  'TSUnionType', 'TSIntersectionType', 'TSParenthesizedType', 'TSTypeParameterInstantiation',
]);

const NOT_CHILDREN = new Set([
  'loc', 'start', 'end', 'range', 'extra', 'leadingComments', 'trailingComments', 'innerComments',
]);

const isNode = (n) => n !== null && typeof n === 'object' && typeof n.type === 'string';
const isRelative = (src) => src.startsWith('.');

function childNodes(node) {
  const out = [];
  for (const key of Object.keys(node)) {
    if (NOT_CHILDREN.has(key)) continue;
    const value = node[key];
    if (Array.isArray(value)) out.push(...value.filter(isNode));
    else if (isNode(value)) out.push(value);
  }
  return out;
}

function* allNodes(value) {
  if (Array.isArray(value)) {
    for (const item of value) yield* allNodes(item);
    return;
  }
  if (!isNode(value)) return;
  yield value;
  for (const child of childNodes(value)) yield* allNodes(child);
}

// Vue's own rule: a node whose leading comment contains @vue-ignore is not read (its type is unknown on purpose).
function hasVueIgnore(node) {
  return !!node.leadingComments?.some((c) => c.value.includes('@vue-ignore'));
}

function typeParamNames(node) {
  const decl = node.typeParameters;
  if (decl?.type !== 'TSTypeParameterDeclaration') return [];
  return decl.params.map((p) => (typeof p.name === 'string' ? p.name : p.name.name));
}

// The identifier at the start of a name (A in A.B.C), or null.
function leftmost(name) {
  let n = name;
  while (n.type === 'TSQualifiedName') n = n.left;
  return n.type === 'Identifier' ? n : null;
}

function qualifiedPath(name) {
  return name.type === 'TSQualifiedName' ? [...qualifiedPath(name.left), name.right.name] : [name.name];
}

function importedName(spec) {
  return spec.imported.type === 'Identifier' ? spec.imported.name : JSON.stringify(spec.imported.value);
}

// The import declaration that binds a probe's name, as the probe writes it. Named and default imports only.
function probeImport(binding) {
  const from = JSON.stringify(binding.source);
  if (binding.kind === 'default') return `import type ${binding.spec.local.name} from ${from}`;
  return `import type { ${importedName(binding.spec)} as ${binding.spec.local.name} } from ${from}`;
}

// The project file that a relative type import names, recorded as a dependency of this component. The dependency is
// recorded here because compiler-sfc's cache hides the reads of a file from a later compile in the same process.
function recordTypeFile(ctx, src) {
  const file = ctx.resolveTypeImport(src);
  if (file !== null) ctx.addDependency(ctx.realDependency(file));
  return file;
}

// True when compileScript fails to read `typeText` as the type of a top-level macro in a component with `prelude`, and
// the failure is at that macro (the error names the probe's own file as its location). A failure inside another file
// is a different error (a declaration the compiler cannot read), and it is not reported here.
function probeFails(ctx, prelude, typeText, memo) {
  const key = `${prelude}\n${typeText}`;
  if (memo.has(key)) return memo.get(key);
  const source = `<script setup lang="ts">\n${prelude ? `${prelude}\n` : ''}defineProps<${typeText}>()\n</script>\n`;
  let fails = false;
  try {
    const { descriptor, errors } = parseSfc(source, { filename: ctx.rel });
    if (!errors.length) {
      compileScript(descriptor, { id: ctx.shortId, isProd: true, fs: ctx.scriptFs({ record: false }) });
    }
  } catch (err) {
    const lines = String(err?.message ?? err).split('\n');
    fails = lines[1] === '' && lines[2] === ctx.rel;
  }
  memo.set(key, fails);
  return fails;
}

// Records each use of an imported binding in a type the macros read. `shadow` holds the type parameters in scope, which
// hide an imported name of the same spelling, as in the compiler.
function visit(node, mode, shadow, cx) {
  if (!isNode(node) || hasVueIgnore(node)) return;
  switch (node.type) {
    case 'TSTypeReference':
      reference(node.typeName, mode, shadow, cx);
      visit(node.typeParameters, mode, shadow, cx);
      return;
    case 'TSExpressionWithTypeArguments':
      reference(node.expression, mode, shadow, cx);
      visit(node.typeParameters, mode, shadow, cx);
      return;
    case 'TSTypeQuery':
      typeQuery(node, shadow, cx);
      return;
    case 'TSImportType':
      if (node.qualifier) {
        cx.uses.push({
          kind: 'inline', node: node.argument, qual: node.qualifier,
          src: node.argument.value, path: qualifiedPath(node.qualifier),
        });
      }
      visit(node.typeParameters, mode, shadow, cx);
      return;
    default: {
      if (mode === 'shape' && !SHAPE_DESCENT.has(node.type)) return;
      const names = typeParamNames(node);
      const inner = names.length ? new Set([...shadow, ...names]) : shadow;
      for (const child of childNodes(node)) visit(child, mode, inner, cx);
    }
  }
}

function reference(name, mode, shadow, cx) {
  const root = leftmost(name);
  if (!root || shadow.has(root.name)) return;
  if (cx.bindings.has(root.name)) {
    cx.uses.push({ kind: 'ref', node: name, path: qualifiedPath(name), binding: cx.bindings.get(root.name) });
  } else if (name.type === 'Identifier' && cx.locals.has(name.name)) {
    follow(name.name, mode, cx);
  }
}

// A local declaration that a macro type names is read as the macro reads it, once per mode.
function follow(name, mode, cx) {
  const key = `${mode} ${name}`;
  if (cx.seen.has(key)) return;
  cx.seen.add(key);
  const decl = cx.locals.get(name);
  const shadow = new Set(typeParamNames(decl));
  if (decl.type === 'TSTypeAliasDeclaration') {
    visit(decl.typeAnnotation, mode, shadow, cx);
    return;
  }
  for (const heritage of decl.extends ?? []) visit(heritage, mode, shadow, cx);
  if (mode === 'all') visit(decl.body, mode, shadow, cx);
}

function typeQuery(node, shadow, cx) {
  const expr = node.exprName;
  if (expr.type === 'TSImportType') {
    cx.uses.push({ kind: 'typeof', node: expr.argument, shown: `import(${JSON.stringify(expr.argument.value)})` });
    return;
  }
  const root = leftmost(expr);
  if (!root || shadow.has(root.name) || !cx.bindings.has(root.name)) return;
  cx.uses.push({ kind: 'typeof', node: root, shown: qualifiedPath(expr).join('.') });
}

function report(ctx, base, node, message, hint) {
  ctx.report('VN6017', ctx.rel, base + node.start, base + node.end, { message, hint });
}

// A node is reported once, however many uses of it the macros make.
function once(cx, node) {
  if (cx.reported.has(node)) return false;
  cx.reported.add(node);
  return true;
}

const BARE_HINT =
  'Declare the type in a .ts file inside the project root and import it from there, or declare it in this file.';
const NO_FILE_HINT = 'Import the type from a .ts file inside the project root, or declare it in this file.';
const TYPEOF_HINT = 'Declare the type by name, in a .ts file inside the project root or in this file.';
const notReadMessage = (shown, src) =>
  `type '${shown}' comes from '${src}', which is not a relative path inside the project, so the compiler cannot read it`;
const notExportedMessage = (shown, src) =>
  `macro type '${shown}' cannot be read from '${src}': the file does not export it as a type`;
const notExportedHint = (src) => `Export the type from '${src}' under this name, or declare it in this file.`;

function judge(ctx, use, base, cx) {
  if (use.kind === 'typeof') {
    if (once(cx, use.node)) {
      report(ctx, base, use.node,
        `typeof '${use.shown}' reads an imported value, which the compiler cannot read in a macro type`, TYPEOF_HINT);
    }
  } else if (use.kind === 'inline') {
    judgeInline(ctx, use, base, cx);
  } else {
    judgeBinding(ctx, use, base, cx);
  }
}

function judgeInline(ctx, use, base, cx) {
  const shown = use.path.join('.');
  if (!isRelative(use.src)) {
    if (once(cx, use.node)) report(ctx, base, use.node, notReadMessage(shown, use.src), BARE_HINT);
  } else if (!recordTypeFile(ctx, use.src)) {
    if (once(cx, use.node)) {
      report(ctx, base, use.node, `type import from '${use.src}' names no file in the project`, NO_FILE_HINT);
    }
  } else if (!cx.reported.has(use.qual) && probeFails(ctx, '', `import(${JSON.stringify(use.src)}).${shown}`, cx.memo)) {
    once(cx, use.qual);
    report(ctx, base, use.qual, notExportedMessage(shown, use.src), notExportedHint(use.src));
  }
}

function judgeBinding(ctx, use, base, cx) {
  const { binding } = use;
  const shown = use.path.join('.');
  if (!isRelative(binding.source)) {
    if (once(cx, binding.decl)) report(ctx, base, binding.decl.source, notReadMessage(shown, binding.source), BARE_HINT);
    return;
  }
  // A relative name that names no file is reported by the import checks of script.mjs (type-only imports) or by the
  // project graph (value imports), so it is not reported again here.
  if (!recordTypeFile(ctx, binding.source)) return;
  if (binding.kind === 'namespace') {
    if (once(cx, binding.spec)) {
      const message = `macro type '${shown}' goes through the namespace import '${binding.spec.local.name}', ` +
        'which the compiler does not read';
      report(ctx, base, binding.spec.local, message,
        `Import each type by name, for example: import type { Name } from '${binding.source}'.`);
    }
    return;
  }
  if (cx.reported.has(binding.spec) || !probeFails(ctx, probeImport(binding), shown, cx.memo)) return;
  once(cx, binding.spec);
  report(ctx, base, binding.kind === 'named' ? binding.spec.imported : binding.spec.local,
    notExportedMessage(shown, binding.source), notExportedHint(binding.source));
}

// Checks the imports that the macros of a <script setup> block read. `body` is the block's program body and `base` the
// offset of the block in the .vue file.
export function checkMacroTypes(ctx, body, base) {
  const locals = new Map();
  const bindings = new Map();
  for (const node of body) {
    if (node.type === 'TSTypeAliasDeclaration' || node.type === 'TSInterfaceDeclaration') {
      locals.set(node.id.name, node);
    } else if (node.type === 'ImportDeclaration') {
      for (const spec of node.specifiers) {
        const kind = spec.type === 'ImportDefaultSpecifier' ? 'default'
          : spec.type === 'ImportNamespaceSpecifier' ? 'namespace' : 'named';
        bindings.set(spec.local.name, { decl: node, spec, kind, source: node.source.value });
      }
    }
  }
  const cx = { locals, bindings, uses: [], seen: new Set(), reported: new Set(), memo: new Map() };
  for (const node of allNodes(body)) {
    const mode = node.type === 'CallExpression' && node.callee.type === 'Identifier'
      ? TYPE_MACROS.get(node.callee.name) : undefined;
    if (mode === undefined) continue;
    for (const arg of node.typeParameters?.params ?? []) visit(arg, mode, new Set(), cx);
  }
  for (const use of cx.uses) judge(ctx, use, base, cx);
}
