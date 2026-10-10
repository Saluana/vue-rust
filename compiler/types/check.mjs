// Type checking for vue-native check (tasks 5.7 and 2.2). The project's modules are checked as one TypeScript program:
// TypeScript 6.0.3 with the Vue language tools (vue-tsc 3.3.12). Each .vue file is checked as the virtual TypeScript file
// that @vue/language-core generates, and every reported span is mapped back to the .vue text (pair.mjs pins the versions).
//   typeCheckProject({ root, modules, entry, target, frameworkRoot }) -> { versions, diagnostics }
//   root: absolute project root. modules: root-relative module ids ('/src/App.vue'), as compileProject names them.
//   diagnostics: VN7001 Diagnostic objects (compiler/diagnostics), file names root-relative ('src/App.vue').
// Settings: strict, ES modules, no emit. JavaScript modules are not type checked (allowJs without checkJs), so a .js or
// a JavaScript .vue file reports no type errors. verbatimModuleSyntax and isolatedModules: a type imported as a value
// is an error, as it would be at run time. `vue` resolves to the framework's pinned copy in node_modules. The DOM library
// is included for the web target only (design S23: native libraries omit DOM and Node).
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { createDiagnostic, createLineMap } from '../diagnostics/diagnostic.mjs';
import { FRAMEWORK_DEFAULT, checkToolingPair, resolvePackage } from './pair.mjs';

export class TypeCheckError extends Error {
  constructor(message) {
    super(message);
    this.name = 'TypeCheckError';
  }
}

const toPosix = (p) => p.split(path.sep).join('/');
// True when file lies inside root (not root itself, not a parent or an unrelated absolute path).
const isInside = (root, file) => {
  const rel = path.relative(root, file);
  return rel !== '' && !rel.startsWith('..') && !path.isAbsolute(rel);
};

function compilerOptions(ts, { target, vueDir }) {
  return {
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    lib: target === 'web' ? ['lib.es2022.d.ts', 'lib.dom.d.ts'] : ['lib.es2022.d.ts'],
    types: [],
    paths: { vue: [vueDir] },
    strict: true,
    noEmit: true,
    skipLibCheck: true,
    allowJs: true,
    checkJs: false,
    isolatedModules: true,
    verbatimModuleSyntax: true,
    allowImportingTsExtensions: true,
    // .vue files are root files of the program. TypeScript accepts them only with this option, which Volar's own
    // protocol layer also sets (@volar/typescript/lib/protocol/createProject.js).
    allowNonTsExtensions: true,
  };
}

// One VN7001 diagnostic per TypeScript error or warning, positioned in the .vue (or .ts) text it came from. Suggestions
// and messages are not reported. A diagnostic in a file outside the project (a library file) is attached to the entry
// module and its message names that file.
function toDiagnostics(ts, raw, { root, fallback }) {
  const sources = new Map();
  const sourceOf = (file, text) => {
    if (!sources.has(file)) sources.set(file, { text, map: createLineMap(text) });
    return sources.get(file);
  };
  const out = [];
  for (const d of raw) {
    const severity = d.category === ts.DiagnosticCategory.Error ? 'error'
      : d.category === ts.DiagnosticCategory.Warning ? 'warning' : null;
    if (severity === null) continue;
    const inside = d.file && isInside(root, d.file.fileName);
    let file = fallback;
    let source;
    let prefix = '';
    if (inside) {
      file = toPosix(path.relative(root, d.file.fileName));
      source = sourceOf(file, d.file.text);
    } else {
      source = sourceOf(fallback, readFileSync(path.join(root, fallback), 'utf8'));
      if (d.file) prefix = `${toPosix(path.relative(root, d.file.fileName))}: `;
    }
    const start = Math.min(Math.max(0, d.start ?? 0), source.text.length);
    const end = Math.min(start + Math.max(0, d.length ?? 0), source.text.length);
    out.push({
      file,
      severity,
      message: `${prefix}TS${d.code}: ${ts.flattenDiagnosticMessageText(d.messageText, '\n')}`,
      start: source.map.posAt(start),
      end: source.map.posAt(end),
    });
  }
  const keyOf = (d) => [d.file, d.start.offset, d.end.offset, d.severity, d.message].join('\u0000');
  const seen = new Set();
  const unique = out.filter((d) => (seen.has(keyOf(d)) ? false : seen.add(keyOf(d))));
  unique.sort((a, b) => (a.file < b.file ? -1 : a.file > b.file ? 1 : 0) || a.start.offset - b.start.offset ||
    a.end.offset - b.end.offset || (a.message < b.message ? -1 : a.message > b.message ? 1 : 0));
  return unique.map((d) => createDiagnostic('VN7001', {
    message: d.message,
    severity: d.severity,
    file: d.file,
    start: d.start,
    end: d.end,
  }));
}

// Type checks the project. Throws TypeCheckError when the installed tooling is not the pinned pair or vue is missing:
// the check then does not run at all, so no result comes from another checker.
export function typeCheckProject({ root, modules, entry = modules[0], target = 'web', frameworkRoot = FRAMEWORK_DEFAULT }) {
  if (!path.isAbsolute(root)) throw new TypeError('typeCheckProject: root must be an absolute path');
  if (!['native', 'web'].includes(target)) throw new TypeError(`typeCheckProject: target must be native or web, got ${String(target)}`);
  const pair = checkToolingPair(frameworkRoot);
  if (!pair.ok) throw new TypeCheckError(`type checking needs the pinned TypeScript and Vue language tools: ${pair.problems.join('; ')}`);
  if (modules.length === 0) return { versions: pair.versions, diagnostics: [] };

  const fromFramework = createRequire(path.join(frameworkRoot, 'package.json'));
  const vueTsc = resolvePackage('vue-tsc', frameworkRoot);
  const fromVueTsc = createRequire(path.join(vueTsc.dir, 'package.json'));
  const ts = fromFramework('typescript');
  const core = fromVueTsc('@vue/language-core');
  const { proxyCreateProgram } = fromVueTsc('@volar/typescript');
  const vueDir = resolvePackage('vue', frameworkRoot).dir;

  const vueOptions = core.createParsedCommandLineByJson(ts, ts.sys, root, {}).vueOptions;
  const options = compilerOptions(ts, { target, vueDir });
  const createProgram = proxyCreateProgram(ts, ts.createProgram, (tsApi, opts) => ({
    languagePlugins: [core.createVueLanguagePlugin(tsApi, opts.options, vueOptions, (id) => id)],
  }));
  const program = createProgram({
    rootNames: modules.map((id) => path.join(root, id)),
    options,
    host: ts.createCompilerHost(options),
  });
  const raw = [
    ...program.getOptionsDiagnostics(),
    ...program.getGlobalDiagnostics(),
    ...program.getSyntacticDiagnostics(),
    ...program.getSemanticDiagnostics(),
  ];
  const diagnostics = toDiagnostics(ts, raw, { root, fallback: (entry ?? modules[0]).slice(1) });
  return { versions: pair.versions, diagnostics };
}
