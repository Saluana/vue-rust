// Test helpers for WP-F1 (not a test file). Turns a compiled component module into a page that runs in headless Chrome
// against the real vue@3.5.43 browser build. Types are erased by hand (only @babel/parser is installed, no generator),
// so the erasure supports exactly the TypeScript forms the oracle fixtures use and throws on anything else.
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
export const babel = require('@babel/parser');

export const ROOT = path.resolve(import.meta.dirname, '..', '..', '..');
export const FIXTURES = path.join(ROOT, 'tests', 'sfc', 'fixtures');
export const VUE_GLOBAL = path.join(ROOT, 'node_modules', 'vue', 'dist', 'vue.global.prod.js');

// Applies [start, end) -> replacement edits to source. Edits nested inside an earlier edit are skipped.
function applyEdits(source, edits) {
  const sorted = [...edits].sort((a, b) => a.start - b.start || b.end - a.end);
  let out = '';
  let pos = 0;
  let lastEnd = -1;
  for (const e of sorted) {
    if (e.start < lastEnd) continue;
    out += source.slice(pos, e.start) + e.text;
    pos = e.end;
    lastEnd = e.end;
  }
  return out + source.slice(pos);
}

function walk(node, visit) {
  if (!node || typeof node.type !== 'string') return;
  visit(node);
  for (const key of Object.keys(node)) {
    if (key === 'loc' || key === 'leadingComments' || key === 'trailingComments' || key === 'innerComments') continue;
    const v = node[key];
    if (Array.isArray(v)) for (const c of v) walk(c, visit);
    else if (v && typeof v.type === 'string') walk(v, visit);
  }
}

// Erases TypeScript-only syntax from an ES module that the compiler emitted in lang="ts".
export function eraseTypes(code) {
  const ast = babel.parse(code, { sourceType: 'module', plugins: ['typescript'] }).program;
  const edits = [];
  walk(ast, (n) => {
    switch (n.type) {
      case 'TSTypeAnnotation':
      case 'TSTypeParameterInstantiation':
      case 'TSTypeParameterDeclaration':
        edits.push({ start: n.start, end: n.end, text: '' });
        break;
      case 'TSAsExpression':
      case 'TSSatisfiesExpression': {
        const sl = code.slice(n.expression.end, n.typeAnnotation.start);
        const kw = sl.search(/\b(as|satisfies)\b/);
        edits.push({ start: n.expression.end + kw, end: n.end, text: '' });
        break;
      }
      case 'TSNonNullExpression':
        edits.push({ start: code.indexOf('!', n.expression.end), end: n.end, text: '' });
        break;
      case 'TSInterfaceDeclaration':
      case 'TSTypeAliasDeclaration':
        edits.push({ start: n.start, end: n.end, text: '' });
        break;
      case 'ImportDeclaration':
        if (n.importKind === 'type') edits.push({ start: n.start, end: n.end, text: '' });
        break;
      case 'ExportNamedDeclaration':
        if (n.exportKind === 'type' || (n.declaration && /^TS/.test(n.declaration.type))) {
          edits.push({ start: n.start, end: n.end, text: '' });
        }
        break;
      case 'TSEnumDeclaration':
      case 'TSModuleDeclaration':
      case 'TSParameterProperty':
      case 'TSImportEqualsDeclaration':
        throw new Error(`harness: ${n.type} is not erased by the test helper`);
      default:
        break;
    }
  });
  return applyEdits(code, edits);
}

// Turns an ES module with only `import ... from "vue"`, `export default <expr>` and `export function` into the body of a
// function (Vue) => component. Any other import or export is an error: the oracle fixtures do not use them.
export function moduleToFunction(js) {
  const ast = babel.parse(js, { sourceType: 'module' }).program;
  const edits = [];
  for (const n of ast.body) {
    if (n.type === 'ImportDeclaration') {
      if (n.source.value !== 'vue') throw new Error(`harness: import from ${n.source.value} is not available in the page`);
      const names = n.specifiers.map((s) => {
        if (s.type !== 'ImportSpecifier') throw new Error('harness: only named imports from vue are supported');
        const imported = s.imported.type === 'Identifier' ? s.imported.name : s.imported.value;
        return imported === s.local.name ? imported : `${imported}: ${s.local.name}`;
      });
      edits.push({ start: n.start, end: n.end, text: `const { ${names.join(', ')} } = Vue;` });
    } else if (n.type === 'ExportDefaultDeclaration') {
      edits.push({ start: n.start, end: n.end, text: `return ${js.slice(n.declaration.start, n.declaration.end)};` });
    } else if (n.type === 'ExportNamedDeclaration') {
      if (!n.declaration || n.declaration.type !== 'FunctionDeclaration') {
        throw new Error(`harness: unsupported named export ${n.type}`);
      }
      edits.push({ start: n.start, end: n.end, text: js.slice(n.declaration.start, n.declaration.end) });
    }
  }
  return `(function (Vue) {\n${applyEdits(js, edits)}\n})`;
}

// Full page for a compiled component. wrapper is a JavaScript object-literal source for the root component; it can
// use Vue and comp (the component) and returns the component options. The page defines window.__vn helpers:
//   html()                  innerHTML of #app
//   tick()                  waits for Vue's scheduler, then returns html()
//   click(selector), input(selector, value), change(selector, checked), select(selector, value)
//   call(fnSource)          runs fnSource(Vue, root instance proxy) and returns its result
export function pageFor(compiledCode, lang, wrapper, css = '') {
  let js = lang === 'ts' ? eraseTypes(compiledCode) : compiledCode;
  const fnSource = moduleToFunction(js);
  const vueSource = fs.readFileSync(VUE_GLOBAL, 'utf8');
  if (vueSource.includes('</script') || css.includes('</style')) throw new Error('harness: unsafe inline text');
  return `<!doctype html><meta charset="utf-8"><style>${css}</style><div id="app"></div>
<script>${vueSource}</script>
<script>
(function () {
  const Vue = window.Vue;
  const comp = (${fnSource})(Vue);
  const root = (${wrapper});
  const app = Vue.createApp(root);
  window.__vnApp = app;
  app.mount('#app');
  const el = () => document.getElementById('app');
  const tick = () => Vue.nextTick().then(() => el().innerHTML);
  window.__vn = {
    html: () => el().innerHTML,
    tick,
    click(selector) { document.querySelector(selector).click(); return tick(); },
    input(selector, value) {
      const node = document.querySelector(selector);
      node.value = value;
      node.dispatchEvent(new Event('input', { bubbles: true }));
      return tick();
    },
    change(selector, checked) {
      const node = document.querySelector(selector);
      node.checked = checked;
      node.dispatchEvent(new Event('change', { bubbles: true }));
      return tick();
    },
    select(selector, value) {
      const node = document.querySelector(selector);
      node.value = value;
      node.dispatchEvent(new Event('change', { bubbles: true }));
      return tick();
    },
    call(fn) { return fn(Vue, window.__vnRef); },
  };
})();
</script>`;
}
