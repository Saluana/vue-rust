// Script block policy and profile checks (WP-F1 steps 2, 3 and 5): lang and src rules, exports in <script setup>,
// the Options API form of a normal <script>, imports from vue and node built-ins, type-only imports that name no file
// (VN6017), imports that a macro's type argument cannot be read through (macro-types.mjs, VN6017), reserved names, and
// the profile checks of the user's own code (checkSource: parse, scope analysis, profile rules), mapped back to the
// .vue file.
import { checkSource } from '../frontend/check.mjs';
import { checkStdlib } from '../analysis/stdlib-check.mjs';
import { isNodeBuiltin, manifestEntry, vueSupported } from './context.mjs';
import { checkMacroTypes } from './macro-types.mjs';

const EXPORT_TYPES = new Set(['ExportNamedDeclaration', 'ExportDefaultDeclaration', 'ExportAllDeclaration']);

// vue functions whose result is a reactive proxy. Vue rebinds a const holding one as let when v-model writes it.
const REACTIVE_CALLS = new Set(['reactive', 'readonly', 'shallowReactive', 'shallowReadonly']);

function propKey(prop) {
  if (prop.type !== 'ObjectProperty' && prop.type !== 'ObjectMethod') return null;
  if (prop.computed) return null;
  if (prop.key.type === 'Identifier') return prop.key.name;
  if (prop.key.type === 'StringLiteral') return prop.key.value;
  return null;
}

// A type-only import of a relative module must name a file in the project (VN6017). Vue's type resolver does not fail
// on such a module where a prop's type is only read: the prop is typed as unknown and no error is reported. Value
// imports of missing files are reported by the project graph (VN9001).
function checkTypeImport(ctx, node, base) {
  const src = node.source.value;
  if (!src.startsWith('./') && !src.startsWith('../')) return;
  const typeOnly = (spec) => spec.importKind === 'type' || spec.importKind === 'typeof';
  const erased = typeOnly(node) || (node.specifiers.length > 0 && node.specifiers.every(typeOnly));
  if (!erased || ctx.resolveTypeImport(src) !== null) return;
  ctx.report('VN6017', ctx.rel, base + node.source.start, base + node.source.end, {
    message: `type import from '${src}' names no file in the project`,
  });
}

function checkImport(ctx, node, base) {
  const src = node.source.value;
  if (src === 'vue') {
    for (const spec of node.specifiers) {
      const span = [base + spec.start, base + spec.end];
      if (spec.type === 'ImportDefaultSpecifier' || spec.type === 'ImportNamespaceSpecifier') {
        ctx.report('VN6202', ctx.rel, ...span, {
          message: `${spec.type === 'ImportDefaultSpecifier' ? 'default' : 'namespace'} import from 'vue' is not supported`,
        });
        continue;
      }
      const name = spec.imported.type === 'Identifier' ? spec.imported.name : spec.imported.value;
      if (node.importKind === 'type' || spec.importKind === 'type') {
        const st = manifestEntry('type', name);
        if (st && st.status === 'X') {
          ctx.report('VN6201', ctx.rel, ...span, {
            message: `type '${name}' of 'vue' is not supported`,
            hint: st.note ?? undefined,
          });
        }
        continue;
      }
      if (!vueSupported(name, ctx.target)) {
        const st = manifestEntry('vue', name);
        ctx.report('VN6201', ctx.rel, ...span, {
          message: `export '${name}' of 'vue' is not in the profile`,
          hint: st?.note ?? undefined,
        });
      } else if (manifestEntry('macro', name) && spec.local.name === name) {
        // The official compiler prints this once per process with console.warn; reporting it here keeps it in the result.
        ctx.report('VN6016', ctx.rel, ...span, {
          message: `'${name}' is a compiler macro and does not need to be imported`,
          severity: 'warning',
        });
      }
    }
  } else if (src.startsWith('vue/')) {
    ctx.report('VN6203', ctx.rel, base + node.start, base + node.end, {
      message: `import from '${src}' is not in the profile`,
    });
  } else if (isNodeBuiltin(src)) {
    ctx.report('VN6204', ctx.rel, base + node.source.start, base + node.source.end, {
      message: `Node built-in module '${src}' is not in the profile`,
    });
  }
}

// Default export of a normal <script> (S02, A10, A18). Accepted: an object literal whose entries are name (a string
// literal) and inheritAttrs (a boolean literal); defineComponent of such an object literal, which may also have setup
// (A10); or defineComponent of one function literal, the function form, which is the setup function. A setup entry is
// a method, a function literal or a name, and it is not async or a generator (async setup needs Suspense, S14). Any
// other entry is reported at the entry: an Options API option (A18), or an entry that cannot be read statically. Any
// other default export is reported at the whole export statement.
const isFunctionNode = (n) => n.type === 'ArrowFunctionExpression' || n.type === 'FunctionExpression';
const isPlainFunction = (n) => isFunctionNode(n) && !n.async && !n.generator;

// True when callee is a local name that this block imports from 'vue' under the imported name `name`.
function isVueImport(body, callee, name) {
  if (callee.type !== 'Identifier') return false;
  return body.some((node) => node.type === 'ImportDeclaration' && node.source.value === 'vue' && node.importKind !== 'type' &&
    node.specifiers.some((spec) => spec.type === 'ImportSpecifier' && spec.importKind !== 'type' &&
      spec.local.name === callee.name && (spec.imported.type === 'Identifier' ? spec.imported.name : spec.imported.value) === name));
}

// A setup entry of defineComponent's object: a method, a plain function literal or a name (its value is not known here).
function isSetupEntry(prop) {
  if (prop.type === 'ObjectMethod') return prop.kind === 'method' && !prop.async && !prop.generator;
  if (prop.type !== 'ObjectProperty' || prop.computed) return false;
  return prop.value.type === 'Identifier' || isPlainFunction(prop.value);
}

function checkDefaultExport(ctx, body, base) {
  const exp = body.find((n) => n.type === 'ExportDefaultDeclaration');
  if (!exp) return;
  const span = [base + exp.start, base + exp.end];
  let decl = exp.declaration;
  let wrapped = false;
  if (decl.type === 'CallExpression') {
    const [arg] = decl.arguments;
    if (!isVueImport(body, decl.callee, 'defineComponent') || decl.arguments.length !== 1 || !arg ||
        !(arg.type === 'ObjectExpression' || isPlainFunction(arg))) {
      ctx.report('VN6007', ctx.rel, ...span, {
        message: 'the default export must be an object literal, or defineComponent of an object literal or of one setup function; Options API is excluded',
      });
      return;
    }
    if (arg.type !== 'ObjectExpression') return; // the function form: the function is the setup function (A10)
    decl = arg;
    wrapped = true;
  }
  if (decl.type !== 'ObjectExpression') {
    ctx.report('VN6007', ctx.rel, ...span, {
      message: 'the default export must be an object literal with name or inheritAttrs; Options API is excluded',
    });
    return;
  }
  const allowed = wrapped ? 'name, inheritAttrs and setup' : 'name and inheritAttrs';
  for (const prop of decl.properties) {
    const key = propKey(prop);
    const ok = (key === 'setup' && wrapped) ? isSetupEntry(prop) : (prop.type === 'ObjectProperty' && !prop.computed && (
      (key === 'name' && prop.value.type === 'StringLiteral') ||
      (key === 'inheritAttrs' && prop.value.type === 'BooleanLiteral')
    ));
    if (ok) continue;
    const opt = key === null ? null : manifestEntry('option', key);
    let message;
    let hint = opt?.note ?? undefined;
    if (key === 'setup' && wrapped) {
      message = 'setup must be a function or a name bound to one, and not async or a generator (async setup needs Suspense, S14)';
      hint = 'Write setup as a synchronous function that returns the state object or a render function.';
    } else if (opt) message = `Options API option '${key}' is not supported`;
    else if (key === 'name') message = "'name' must be a string literal";
    else if (key === 'inheritAttrs') message = "'inheritAttrs' must be a boolean literal";
    else if (key === null) message = `this default export entry is not supported; only ${allowed} are allowed`;
    else message = `default export property '${key}' is not supported; only ${allowed} are allowed`;
    ctx.report('VN6007', ctx.rel, base + prop.start, base + prop.end, { message, hint });
  }
}

// Checks one script block. role is 'setup' or 'normal'. reserveRender is true when the component is compiled as a
// module that exports render (a template without <script setup>); then a top-level render binding would clash with it.
// Returns: { role, lang, broken, names, sources, reactiveConsts } (reactiveConsts: const names initialised by reactive(),
// readonly() and their shallow forms, which v-model may not write)
// broken means the block could not be analysed; name resolution for templates is then skipped, because the
// missing bindings would only add errors on top of the one already reported.
export function checkScript(ctx, block, role, { reserveRender }) {
  const lang = block.lang ?? 'js';
  const out = { role, lang, broken: false, names: new Set(), sources: new Set(), reactiveConsts: new Set() };
  if (lang !== 'js' && lang !== 'ts') {
    ctx.report('VN6006', ctx.rel, ...ctx.blockSpan(block), { message: `lang="${lang}" is not supported for <script>` });
    out.broken = true;
    return out;
  }
  if (block.src) {
    ctx.report('VN6005', ctx.rel, ...ctx.blockSpan(block), {
      message: `<${role === 'setup' ? 'script setup' : 'script'} src> is not supported; the script must be inline`,
    });
    out.broken = true;
    return out;
  }

  const base = block.loc.start.offset;
  const res = checkSource(block.content, { filename: ctx.rel, lang, target: ctx.target });
  if (res.ast && !res.diagnostics.some(d => d.severity === 'error')) {
    res.diagnostics.push(...checkStdlib(res.ast, { source: block.content, filename: ctx.rel, target: ctx.target, analysis: res.analysis }));
  }
  for (const d of res.diagnostics) {
    // Setup macros are compiler bindings; compileScript validates and removes their calls.
    const ref = res.analysis?.references.find(r => r.start === d.start.offset && r.binding === -1);
    if (role === 'setup' && d.code === 'VN4016' && ref && manifestEntry('macro', ref.name)) continue;
    ctx.report(d.code, ctx.rel, base + d.start.offset, base + d.end.offset, {
      message: d.message,
      hint: d.hint,
      severity: d.severity,
    });
  }
  if (!res.ast) {
    out.broken = true;
    return out;
  }
  const body = res.ast.program.body;

  if (role === 'setup') {
    for (const node of body) {
      if (EXPORT_TYPES.has(node.type)) {
        ctx.report('VN6009', ctx.rel, base + node.start, base + node.end, {
          message: '<script setup> cannot contain ES module exports',
        });
      }
    }
  } else {
    checkDefaultExport(ctx, body, base);
  }

  for (const node of body) {
    if (node.type !== 'VariableDeclaration' || node.kind !== 'const') continue;
    for (const d of node.declarations) {
      const init = d.init;
      if (d.id.type === 'Identifier' && init?.type === 'CallExpression' && init.callee.type === 'Identifier' &&
          REACTIVE_CALLS.has(init.callee.name)) {
        out.reactiveConsts.add(d.id.name);
      }
    }
  }

  for (const node of body) {
    if (node.type === 'ImportDeclaration') {
      checkImport(ctx, node, base);
      checkTypeImport(ctx, node, base);
      out.sources.add(node.source.value);
    }
  }
  if (role === 'setup') checkMacroTypes(ctx, body, base);

  const module = res.analysis.module;
  for (const id of module.bindingIds) {
    const b = res.analysis.bindings[id];
    out.names.add(b.name);
    const reserved = b.name === '__sfc' || (b.name === 'render' && reserveRender);
    if (reserved) {
      ctx.report('VN6011', ctx.rel, base + b.start, base + b.end, {
        message: `'${b.name}' is reserved by the generated component`,
      });
    }
  }
  return out;
}
