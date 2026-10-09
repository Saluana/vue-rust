// Project compilation (WP-C1c): a graph of JavaScript, TypeScript and .vue modules -> one Rust module per module.
//   compileProject({ entry, files | resolver, target, sfcRootDir }) -> { entryId, modules, natives, diagnostics, ok }
//   Each module: { id, file, lang ('js'|'ts'|'vue'), codeLang ('js'|'ts' of the code that was lowered), kind, origin,
//   rust (null on error), requests, styles, diagnostics }.
//   sfcRootDir (optional, absolute): the directory that external <template src> / <style src> files are read from.
// Module ids are project-root-relative and /-prefixed ('/src/app.js'). A specifier resolves to:
//   - a relative module ('./x.js', '../y.vue') that exists by its exact path (no extension guessing, as in Node ESM);
//   - a native module (natives.mjs): recorded in `natives`, not compiled; the runtime registers it;
//   - nothing else: a VN9001 error (package imports, missing modules, absolute specifiers, unknown file types), or a
//     VN61xx/VN62xx error for vue subpaths, vue default/namespace imports, excluded vue exports and Node built-ins.
// .vue modules go through compileSfc; the generated code is then lowered like any module, and styles are returned.
// Type-only imports and exports are erased (compileModule skips them too). Output is deterministic (sorted ids).
import { checkSource } from '../frontend/check.mjs';
import { compileModule } from '../index.mjs';
import { compileSfc } from '../sfc/index.mjs';
import { createAnyDiagnostic } from '../sfc/codes.mjs';
import { vueSupported, isNodeBuiltin } from '../sfc/context.mjs';
import { createDiagnostic, createLineMap } from '../diagnostics/diagnostic.mjs';
import { isNativeSpecifier } from './natives.mjs';

const LANG_OF_EXT = new Map([['.js', 'js'], ['.mjs', 'js'], ['.ts', 'ts'], ['.mts', 'ts'], ['.vue', 'vue']]);

// Normalises a project path to a module id ('/a/b.js'); null when it leaves the project root.
export function toModuleId(p) {
  const out = [];
  for (const seg of String(p).split('/')) {
    if (seg === '' || seg === '.') continue;
    if (seg === '..') {
      if (out.length === 0) return null;
      out.pop();
    } else {
      out.push(seg);
    }
  }
  return out.length === 0 ? null : `/${out.join('/')}`;
}

function dirOf(id) {
  const i = id.lastIndexOf('/');
  return i <= 0 ? '/' : id.slice(0, i);
}

function langOf(id) {
  const dot = id.lastIndexOf('.');
  return dot < 0 ? undefined : LANG_OF_EXT.get(id.slice(dot));
}

function makeReader(files, resolver) {
  if (files !== undefined && resolver !== undefined) throw new TypeError('compileProject: pass options.files or options.resolver, not both');
  if (files !== undefined) {
    const map = new Map();
    const entries = files instanceof Map ? [...files.entries()] : Object.entries(files);
    for (const [key, text] of entries) {
      if (typeof text !== 'string') throw new TypeError(`compileProject: file ${key} must be a string`);
      const id = toModuleId(key);
      if (id === null) throw new TypeError(`compileProject: file ${key} is outside the project root`);
      if (map.has(id)) throw new TypeError(`compileProject: two files map to ${id}`);
      map.set(id, text);
    }
    return (id) => (map.has(id) ? map.get(id) : null);
  }
  if (typeof resolver === 'function') {
    return (id) => {
      const text = resolver(id);
      if (text === null || text === undefined) return null;
      if (typeof text !== 'string') throw new TypeError(`compileProject: resolver(${id}) must return a string or null`);
      return text;
    };
  }
  throw new TypeError('compileProject: options.files or options.resolver is required');
}

// A diagnostic at [start, end) of `source` (UTF-16 offsets), file name `file`. Module-graph errors are VN9001 (the
// unsupported-construct code of the lowering): the registry has no user-facing code for them (see the report).
function spanDiag(code, { file, source, start, end, message, hint, severity = 'error' }) {
  const lines = createLineMap(source);
  const at = (o) => lines.posAt(Math.min(o, source.length));
  if (code.startsWith('VN9')) {
    return createDiagnostic(code, { message, hint, severity, file, start: at(start), end: at(end) });
  }
  return createAnyDiagnostic(code, { message, hint, severity, file, start: at(start), end: at(end) });
}

// Import and re-export sites of a module: { spec, start, end, erased } in source order. A declaration that TypeScript
// erases (import type, export type, or an import whose specifiers are all type-only) is marked erased.
function sitesOf(program) {
  const out = [];
  for (const s of program.body) {
    if (s.type === 'ImportDeclaration') {
      const erased = s.importKind === 'type' || s.importKind === 'typeof' ||
        (s.specifiers.length > 0 && s.specifiers.every((x) => x.importKind === 'type' || x.importKind === 'typeof'));
      out.push({ node: s, spec: s.source.value, start: s.source.start, end: s.source.end, erased, kind: 'import' });
    } else if (s.type === 'ExportAllDeclaration') {
      out.push({ node: s, spec: s.source.value, start: s.source.start, end: s.source.end, erased: s.exportKind === 'type', kind: 'export' });
    } else if (s.type === 'ExportNamedDeclaration' && s.source) {
      const erased = s.exportKind === 'type' || (s.specifiers.length > 0 && s.specifiers.every((x) => x.exportKind === 'type'));
      out.push({ node: s, spec: s.source.value, start: s.source.start, end: s.source.end, erased, kind: 'export' });
    }
  }
  return out;
}

// The vue import policy of the SFC pipeline (VN6201-VN6204), applied to every module of the project.
function vueImportDiagnostics(site, { file, source, target }, diags) {
  const node = site.node;
  if (site.spec.startsWith('vue/')) {
    diags.push(spanDiag('VN6203', { file, source, start: node.start, end: node.end, message: `import from '${site.spec}' is not in the profile` }));
    return;
  }
  if (site.kind !== 'import' || site.spec !== 'vue' || site.erased) return;
  for (const spec of node.specifiers) {
    if (spec.importKind === 'type' || spec.importKind === 'typeof') continue;
    const span = [spec.start, spec.end];
    if (spec.type === 'ImportDefaultSpecifier' || spec.type === 'ImportNamespaceSpecifier') {
      diags.push(spanDiag('VN6202', { file, source, start: span[0], end: span[1], message: `${spec.type === 'ImportDefaultSpecifier' ? 'default' : 'namespace'} import from 'vue' is not supported` }));
      continue;
    }
    const name = spec.imported.type === 'Identifier' ? spec.imported.name : spec.imported.value;
    if (!vueSupported(name, target)) {
      diags.push(spanDiag('VN6201', { file, source, start: span[0], end: span[1], message: `export '${name}' of 'vue' is not in the profile` }));
    }
  }
}

// Compiles the project. See the header for the contract; files map ids to source text.
export function compileProject(options = {}) {
  const { entry, files, resolver, target = 'native', sfcRootDir } = options;
  if (typeof entry !== 'string' || entry === '') throw new TypeError('compileProject: options.entry is required');
  if (!['native', 'web', 'test'].includes(target)) throw new TypeError(`compileProject: target must be "native", "web" or "test", got ${String(target)}`);
  const read = makeReader(files, resolver);
  const entryId = toModuleId(entry);
  if (entryId === null) throw new TypeError(`compileProject: entry ${entry} is outside the project root`);

  const records = new Map();   // id -> record
  const natives = new Set();
  const diagnostics = [];
  const pending = [entryId];

  if (read(entryId) === null) {
    diagnostics.push(spanDiag('VN9001', { file: entryId.slice(1), source: '', start: 0, end: 0, message: `entry module ${entryId} is not in the project` }));
  }

  while (pending.length > 0) {
    const id = pending.shift();
    if (records.has(id)) continue;
    const rec = compileOne(id);
    records.set(id, rec);
    for (const next of rec.loads) if (!records.has(next)) pending.push(next);
  }

  function compileOne(id) {
    const rec = { id, file: id.slice(1), lang: langOf(id), codeLang: null, kind: 'module', origin: 'user', rust: null, requests: [], styles: [], diagnostics: [], loads: [] };
    const own = [];
    const text = read(id);
    if (text === null) return rec;
    const lang = langOf(id);
    if (lang === undefined) {
      own.push(spanDiag('VN9001', { file: rec.file, source: '', start: 0, end: 0, message: `unsupported file type: ${id}` }));
      rec.diagnostics = own;
      diagnostics.push(...own);
      return rec;
    }
    let code = text;
    let codeLang = lang;
    if (lang === 'vue') {
      rec.kind = 'vue';
      rec.origin = 'compiler';
      const sfc = compileSfc(text, sfcRootDir === undefined ? { filename: rec.file, target } : { filename: rec.file, target, rootDir: sfcRootDir });
      own.push(...sfc.diagnostics);
      if (sfc.code === null) {
        rec.diagnostics = own;
        diagnostics.push(...own);
        return rec;
      }
      code = sfc.code;
      codeLang = sfc.lang;
      rec.styles = sfc.styles;
    }
    rec.codeLang = codeLang;
    const parsed = checkSource(code, { filename: rec.file, lang: codeLang, target, stdlib: true });
    own.push(...parsed.diagnostics);
    if (!parsed.ast || parsed.diagnostics.some((d) => d.severity === 'error')) {
      rec.diagnostics = own;
      diagnostics.push(...own);
      return rec;
    }
    const map = new Map();
    let failed = false;
    for (const site of sitesOf(parsed.ast.program)) {
      if (site.erased) continue;
      // The vue policy applies to user code. A .vue module's generated code imports the runtime helpers its template
      // needs (openBlock, ...); compileSfc has already checked those imports (origin 'compiler').
      const before = own.length;
      if (rec.origin === 'user') vueImportDiagnostics(site, { file: rec.file, source: code, target }, own);
      if (own.length > before) { failed = true; continue; }
      const { spec } = site;
      if (isNativeSpecifier(spec)) {
        natives.add(spec);
        map.set(spec, spec);
        continue;
      }
      if (spec.startsWith('vue/') || spec === 'vue') continue; // handled above (vue subpaths) or valid
      if (spec.startsWith('node:') || isNodeBuiltin(spec)) {
        own.push(spanDiag('VN6204', { file: rec.file, source: code, start: site.start, end: site.end, message: `Node built-in module '${spec}' is not in the profile` }));
        failed = true;
        continue;
      }
      if (spec.startsWith('./') || spec.startsWith('../') || spec === '.' || spec === '..') {
        const resolved = toModuleId(`${dirOf(id)}/${spec}`);
        if (resolved === null || read(resolved) === null) {
          own.push(spanDiag('VN9001', { file: rec.file, source: code, start: site.start, end: site.end, message: `module '${spec}' is not in the project` }));
          failed = true;
          continue;
        }
        map.set(spec, resolved);
        rec.loads.push(resolved);
        continue;
      }
      if (spec.startsWith('/')) {
        own.push(spanDiag('VN9001', { file: rec.file, source: code, start: site.start, end: site.end, message: `absolute module specifier '${spec}' is not supported` }));
        failed = true;
        continue;
      }
      own.push(spanDiag('VN9001', { file: rec.file, source: code, start: site.start, end: site.end, message: `package import '${spec}' is outside the closed world (only native modules and project files can be imported)` }));
      failed = true;
    }
    rec.requests = [];
    if (failed || own.some((d) => d.severity === 'error')) {
      rec.diagnostics = own;
      diagnostics.push(...own);
      return rec;
    }
    const c = compileModule(code, {
      filename: rec.file,
      lang: codeLang,
      specifier: id,
      origin: rec.origin,
      target,
      resolve: (s) => (map.has(s) ? map.get(s) : s),
    });
    own.push(...c.diagnostics);
    rec.rust = c.rust;
    rec.requests = c.requests;
    rec.diagnostics = own;
    diagnostics.push(...own);
    return rec;
  }

  const modules = [...records.values()]
    .sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
    .map(({ loads, ...m }) => m);
  const ok = diagnostics.every((d) => d.severity !== 'error');
  return {
    entryId,
    modules,
    natives: [...natives].sort(),
    diagnostics,
    ok,
  };
}
