// .vue single-file component frontend (WP-F1). compileSfc wraps the official @vue/compiler-sfc 3.5.43 with the
// language-profile checks, native template validation, style extraction and .vue-mapped diagnostics:
//
//   compileSfc(source, { filename, id?, target: 'native'|'web'|'test', rootDir?, trustedHtml? })
//     -> { code, map, styles, bindings, diagnostics, dependencies, scopeId, lang, sources }
//
// Pipeline: parse blocks (official) -> block policy -> script profile checks -> template validation on the untouched
// AST -> style compile -> (only when nothing failed) official compileScript / compileTemplate -> generated-import check.
// Diagnostics carry VN6xxx codes (compiler/sfc/codes.mjs) or frontend codes, with positions in the .vue file (or in
// the template/style src file they point into). code, map and styles are null/empty when any error diagnostic exists.
// Determinism: output has no timestamps and no absolute paths; file names are relative to rootDir.
//
// Output shape (code, lang):
//   <script setup>: compileScript output with the template inlined in setup(), named __sfc (genDefaultAs), then
//     `__sfc.__scopeId = "data-v-<id>";` when a scoped style exists, then `export default __sfc;`.
//   no <script setup>: the template compiled as a module (`export function render(_ctx, _cache)`), the normal <script>
//     with its default export rewritten to `const __sfc = {...}` (or `{}`), `__sfc.render = render;`, the scope id
//     when scoped, and `export default __sfc;`. map is null for this form (no combined map is built).
//   Imports in code are from 'vue' (names checked with origin 'compiler') and from the user's own blocks only.
//   lang is 'ts' when a script block is TypeScript; the code is then TypeScript, not transpiled.
// dependencies: project-relative files this compile read (template src, style src, files read for type resolution).
//   User imports are not listed; the build graph owns module imports.
import { parse as parseSfc, compileScript, compileTemplate, rewriteDefault, parseCache } from '@vue/compiler-sfc';
import { parse as parseDom } from '@vue/compiler-dom';
import { parseModule } from '../frontend/parse.mjs';
import { formatDiagnostic } from '../diagnostics/format.mjs';
import { SFC_CODES, sfcCodeInfo } from './codes.mjs';
import {
  GENERATOR, VUE_REFERENCE, SfcContext, manifestEntry, realInsideRoot, resolveOptions, vueCompilerSupported,
} from './context.mjs';
import { checkScript } from './script.mjs';
import { validateTemplate } from './template.mjs';
import { compileStyles } from './styles.mjs';

export { SFC_CODES, sfcCodeInfo };
export { VUE_REFERENCE, GENERATOR };

const isNativeTag = (tag) => manifestEntry('native-tag', tag) !== null;

// Options of the template parser (parse of the SFC blocks and of template src files).
const TEMPLATE_PARSE = Object.freeze({ isNativeTag, isCustomElement: () => false, whitespace: 'condense' });

// Options of the template compiler. The reasons:
//   mode: 'module'            ES module output (compileScript's inline mode ignores it)
//   prefixIdentifiers: true   identifiers are resolved against the setup scope, as Vue's own SFC plugin does
//   hoistStatic: false        static vnodes are not hoisted (createStaticVNode and insertStaticContent are unsupported)
//   cacheHandlers: true       stable handler functions, as in production builds
//   whitespace: 'condense'    Vue's default whitespace policy
//   isNativeTag               the manifest's native tag set (including excluded tags, so they get VN6102, not a component)
//   isCustomElement: false    custom elements are excluded (U19, U32)
//   transformAssetUrls: false relative URLs stay plain strings (asset imports are a later WP)
const TEMPLATE_COMPILE = Object.freeze({
  mode: 'module',
  prefixIdentifiers: true,
  hoistStatic: false,
  cacheHandlers: true,
  whitespace: 'condense',
  isNativeTag,
  isCustomElement: () => false,
});

function headerFor(ctx) {
  return `/* ${GENERATOR}; Vue ${VUE_REFERENCE}; source ${ctx.rel} */`;
}

function parseErrorCode(message) {
  if (/can contain only one/.test(message)) return 'VN6002';
  if (/cannot use the "src" attribute/.test(message)) return 'VN6005';
  if (/Error parsing JavaScript expression/.test(message)) return 'VN1000';
  if (/At least one <template> or <script> is required/.test(message)) return 'VN6010';
  return 'VN6001';
}

function errorLoc(err) {
  const loc = err && err.loc;
  if (loc && loc.start && Number.isInteger(loc.start.offset)) {
    return [loc.start.offset, Number.isInteger(loc.end?.offset) ? loc.end.offset : loc.start.offset];
  }
  return null;
}

function firstLine(message) {
  return String(message).split('\n')[0];
}

function sortDiagnostics(diags) {
  const seen = new Set();
  const out = [];
  for (const d of diags) {
    const key = [d.file, d.code, d.severity, d.start.offset, d.end.offset, d.message].join('\u0000');
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(d);
  }
  const cmp = (a, b) => (a < b ? -1 : a > b ? 1 : 0);
  out.sort((a, b) => cmp(a.file, b.file) || a.start.offset - b.start.offset || a.end.offset - b.end.offset ||
    cmp(a.code, b.code) || cmp(a.message, b.message));
  return out;
}

// Final result. On any error diagnostic the outputs are withdrawn; diagnostics and dependencies are kept.
function finish(ctx, { code = null, map = null, styles = [], bindings = {}, lang = 'js' } = {}) {
  const diagnostics = sortDiagnostics(ctx.diags);
  const failed = diagnostics.some((d) => d.severity === 'error');
  const sources = {};
  for (const [file, entry] of ctx.texts) {
    if (file !== ctx.rel) sources[file] = entry.text;
  }
  return {
    code: failed ? null : code,
    map: failed ? null : map,
    styles: failed ? [] : styles,
    bindings: failed ? {} : bindings,
    diagnostics,
    dependencies: [...ctx.deps].sort(),
    scopeId: ctx.scopeId,
    lang: failed ? 'js' : lang,
    sources,
  };
}

// Vue reports some block errors as a zero-width location at the "<" of the block. Such a span is widened to the
// opening tag, so that the caret shows the block the message is about.
function reportParseErrors(ctx, errors, file) {
  const text = ctx.texts.get(file).text;
  for (const err of errors) {
    const message = firstLine(err.message);
    let [s, e] = errorLoc(err) ?? [0, 0];
    if (s === e && text[s] === '<') {
      const close = text.indexOf('>', s);
      if (close > s) e = close + 1;
    }
    ctx.report(parseErrorCode(message), file, s, e, { message });
  }
}

// Template compile errors from the official compiler. base converts the offsets of that compile to file offsets.
function reportTemplateErrors(ctx, errors, file, base, fallbackSpan) {
  for (const err of errors) {
    if (typeof err === 'string') {
      ctx.report('VN6013', file, ...fallbackSpan, { message: firstLine(err) });
      continue;
    }
    const loc = errorLoc(err);
    if (loc) ctx.report('VN6013', file, base + loc[0], base + loc[1], { message: firstLine(err.message) });
    else ctx.report('VN6013', file, ...fallbackSpan, { message: firstLine(err.message) });
  }
}

// Turns an exception of the official compiler into a diagnostic. Errors with a numeric code are template compile
// errors (CompilerError); the others come from compileScript and carry no location, so the block tag is the span.
function reportCompilerException(ctx, err, block, file) {
  const message = firstLine(err && err.message ? err.message : err);
  if (typeof err?.code === 'number') {
    const loc = errorLoc(err);
    const span = loc ?? ctx.blockSpan(block);
    ctx.report('VN6013', file, span[0], span[1], { message });
  } else {
    const typesHint = /Failed to resolve import source/.test(message)
      ? 'Imported types are resolved with the typescript package, which this frontend does not install. Declare the props in this file.'
      : undefined;
    ctx.report('VN6012', file, ...ctx.blockSpan(block), { message, hint: typesHint });
  }
}

// Setup components: compileScript inlines the template into setup(); the component object is named __sfc so that
// __scopeId can be set before the default export, as Vue's own plugin does.
function compileSetup(ctx, descriptor, { anyScoped }) {
  let out;
  try {
    out = compileScript(descriptor, {
      id: ctx.shortId,
      isProd: true,
      sourceMap: true,
      inlineTemplate: true,
      genDefaultAs: '__sfc',
      fs: ctx.scriptFs(),
      templateOptions: { compilerOptions: TEMPLATE_COMPILE, transformAssetUrls: false, isProd: true },
    });
  } catch (err) {
    reportCompilerException(ctx, err, descriptor.scriptSetup, ctx.rel);
    return null;
  }
  for (const dep of out.deps ?? []) {
    const real = realInsideRoot(dep, ctx.rootReal);
    if (real) ctx.addDependency(real);
  }
  const parts = [headerFor(ctx), out.content];
  if (anyScoped) parts.push(`__sfc.__scopeId = ${JSON.stringify(ctx.scopeId)};`);
  parts.push('export default __sfc;');
  return { code: parts.join('\n') + '\n', map: out.map ?? null, bindings: out.bindings ?? {}, lang: null };
}

// Components without <script setup>: the template is compiled as a module that exports render; a normal <script>
// (object literal with name and inheritAttrs, or nothing) supplies the component object.
function compileNormal(ctx, descriptor, { anyScoped, templateFile }) {
  const tpl = descriptor.template;
  const normal = descriptor.script;
  const parts = [headerFor(ctx)];
  let hasRender = false;
  if (tpl) {
    let t;
    try {
      t = compileTemplate({
        source: tpl.content,
        filename: templateFile,
        id: ctx.shortId,
        scoped: anyScoped,
        isProd: true,
        transformAssetUrls: false,
        compilerOptions: TEMPLATE_COMPILE,
      });
    } catch (err) {
      reportCompilerException(ctx, err, tpl, templateFile);
      return null;
    }
    // Offsets of this compile are relative to the template text: inline templates start at the block's content.
    const base = templateFile === ctx.rel ? tpl.loc.start.offset : 0;
    const span = templateFile === ctx.rel ? ctx.blockSpan(tpl) : [0, 0];
    reportTemplateErrors(ctx, t.errors, templateFile, base, span);
    for (const tip of t.tips) {
      ctx.report('VN6016', templateFile, ...span, { message: firstLine(tip), severity: 'warning' });
    }
    if (t.errors.length) return null;
    parts.push(t.code);
    hasRender = true;
  }
  if (normal) {
    try {
      parts.push(rewriteDefault(normal.content, '__sfc', normal.lang === 'ts' ? ['typescript'] : []));
    } catch (err) {
      reportCompilerException(ctx, err, normal, ctx.rel);
      return null;
    }
  } else {
    parts.push('const __sfc = {};');
  }
  if (hasRender) parts.push('__sfc.render = render;');
  if (anyScoped) parts.push(`__sfc.__scopeId = ${JSON.stringify(ctx.scopeId)};`);
  parts.push('export default __sfc;');
  return { code: parts.join('\n') + '\n', map: null, bindings: {}, lang: null };
}

// The generated module may import from 'vue' only names that the official compiler's own output may use (origin
// 'compiler'), and from other modules only what the user's blocks import. Anything else is an internal error.
function checkGenerated(ctx, code, lang, allowedSources) {
  const parsed = parseModule(code, { filename: ctx.rel, lang });
  if (!parsed.ast) {
    ctx.report('VN9001', ctx.rel, 0, 0, { message: `generated module does not parse: ${firstLine(parsed.diagnostics[0].message)}` });
    return false;
  }
  let ok = true;
  for (const node of parsed.ast.program.body) {
    if (node.type !== 'ImportDeclaration') continue;
    const src = node.source.value;
    if (src === 'vue') {
      for (const spec of node.specifiers) {
        if (spec.type !== 'ImportSpecifier') {
          ctx.report('VN9001', ctx.rel, 0, 0, { message: "generated code uses a default or namespace import from 'vue'" });
          ok = false;
          continue;
        }
        const name = spec.imported.type === 'Identifier' ? spec.imported.name : spec.imported.value;
        if (!vueCompilerSupported(name, ctx.target)) {
          ctx.report('VN9001', ctx.rel, 0, 0, {
            message: `generated code imports '${name}' from 'vue', which is outside the profile`,
          });
          ok = false;
        }
      }
    } else if (!allowedSources.has(src)) {
      ctx.report('VN9001', ctx.rel, 0, 0, { message: `generated code imports '${src}', which no user block imports` });
      ok = false;
    }
  }
  return ok;
}

export function compileSfc(source, options = {}) {
  const layout = resolveOptions(source, options);
  const ctx = new SfcContext(layout);
  // compiler-sfc caches parse results by source text, and compileScript rewrites the cached template AST in place.
  // A second compile of the same text would then see the rewritten AST, so the cache is cleared for every compile.
  parseCache.clear();
  const { descriptor, errors } = parseSfc(source, {
    filename: ctx.rel,
    sourceMap: true,
    templateParseOptions: TEMPLATE_PARSE,
  });
  if (errors.length) {
    reportParseErrors(ctx, errors, ctx.rel);
    return finish(ctx);
  }

  // Template src: the file is read inside the project root and parsed on its own (offsets are its own).
  const tpl = descriptor.template;
  let templateFile = ctx.rel;
  let templateBroken = false;
  if (tpl && tpl.src) {
    const f = ctx.readBlockFile(tpl, tpl.src);
    if (!f) {
      templateBroken = true;
    } else {
      const parseErrs = [];
      const root = parseDom(f.text, { parseMode: 'sfc', ...TEMPLATE_PARSE, onError: (e) => parseErrs.push(e) });
      if (parseErrs.length) {
        reportParseErrors(ctx, parseErrs, f.rel);
        templateBroken = true;
      } else {
        templateFile = f.rel;
        tpl.content = f.text;
        tpl.ast = root;
        tpl.src = undefined;
      }
    }
  }

  // Block policy: custom blocks and preprocessor languages are excluded (S04).
  for (const block of descriptor.customBlocks) {
    ctx.report('VN6003', ctx.rel, ...ctx.blockSpan(block), { message: `custom block <${block.type}> is not supported` });
  }
  if (tpl && tpl.lang && tpl.lang !== 'html') {
    ctx.report('VN6004', ctx.rel, ...ctx.blockSpan(tpl), { message: `<template lang="${tpl.lang}"> is not supported` });
    templateBroken = true;
  }
  const setup = descriptor.scriptSetup;
  const normal = descriptor.script;
  const hasTemplate = !!tpl && !templateBroken;
  if (!tpl && !setup && !normal) {
    ctx.report('VN6010', ctx.rel, 0, 0, { message: 'the file has neither a <template> nor a <script> block' });
  }

  // Scripts: profile checks on the user's code. Template component tags resolve against <script setup> bindings
  // (and the normal script's bindings when a setup script exists, since both share one module).
  const reserveRender = hasTemplate && !setup;
  const setupInfo = setup ? checkScript(ctx, setup, 'setup', { reserveRender }) : null;
  const normalInfo = normal ? checkScript(ctx, normal, 'normal', { reserveRender }) : null;
  const scripts = [setupInfo, normalInfo].filter(Boolean);
  const scriptBroken = scripts.some((s) => s.broken);
  const names = new Set();
  if (setupInfo) {
    for (const n of setupInfo.names) names.add(n);
    for (const n of normalInfo?.names ?? []) names.add(n);
  }
  // Type-form defineProps can expose template names without declaring script variables.
  // Use Vue's binding analysis, including imported types, rather than approximating those names.
  if (setup && !scriptBroken && !ctx.diags.some(d => d.severity === 'error')) {
    try {
      const metadata = compileScript(descriptor, { id: ctx.shortId, isProd: true, fs: ctx.scriptFs() });
      for (const name of Object.keys(metadata.bindings ?? {})) {
        if (/^[A-Za-z_$][\w$]*$/.test(name)) names.add(name);
      }
    } catch (err) {
      reportCompilerException(ctx, err, setup, ctx.rel);
    }
  }
  const outLang = scripts.some((s) => s.lang === 'ts') ? 'ts' : 'js';
  const fragmentLang = setupInfo?.lang === 'ts' ? 'ts' : 'js';

  if (hasTemplate && tpl.ast) {
    validateTemplate(ctx, tpl.ast, {
      file: templateFile,
      names,
      scriptBroken,
      lang: fragmentLang,
      reactiveConsts: setupInfo?.reactiveConsts ?? new Set(),
    });
  }

  const styles = compileStyles(ctx, descriptor);
  if (ctx.diags.some((d) => d.severity === 'error')) return finish(ctx, { styles });

  const anyScoped = styles.some((s) => s.scoped);
  const built = setup
    ? compileSetup(ctx, descriptor, { anyScoped })
    : compileNormal(ctx, descriptor, { anyScoped, templateFile: templateFile });
  if (!built) return finish(ctx, { styles });

  const allowedSources = new Set();
  for (const s of scripts) for (const src of s.sources) allowedSources.add(src);
  if (!checkGenerated(ctx, built.code, outLang, allowedSources)) return finish(ctx, { styles });

  return finish(ctx, {
    code: built.code,
    map: built.map,
    styles,
    bindings: built.bindings,
    lang: outLang,
  });
}

// Renders diagnostics with formatDiagnostic (compiler/diagnostics/format.mjs). sources maps file names to their text
// (result.sources), so diagnostics in template or style src files are drawn from their own text; the component file
// uses source.
export function serializeDiagnostics(diagnostics, source, { sources = {} } = {}) {
  return diagnostics.map((d) => formatDiagnostic(d, sources[d.file] ?? source)).join('\n\n');
}
