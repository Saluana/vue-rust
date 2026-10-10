// Per-compilation state for the SFC frontend: option checks, project-relative paths, root-limited file access, the
// diagnostic reporter and the text of every file that diagnostics can point into. Paths in output are always relative
// to the project root, so the result does not depend on the working directory.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { builtinModules } from 'node:module';
import { createLineMap } from '../diagnostics/diagnostic.mjs';
import { lookup, isSupported } from '../profile/index.mjs';
import { createAnyDiagnostic } from './codes.mjs';

export const VUE_REFERENCE = '3.5.43';
export const GENERATOR = 'vue-native SFC frontend (WP-F1, generator 1)';
const TARGETS = new Set(['native', 'web', 'test']);
const ID_RE = /^[A-Za-z0-9_-]+$/;
const NODE_BUILTINS = new Set(builtinModules);

export function toPosix(p) {
  return p.split(path.sep).join('/');
}

// Short, stable scope id: the first 8 hex digits of SHA-256 over the project-relative file name (Vue uses 8 digits too).
export function defaultId(rel) {
  return createHash('sha256').update(rel).digest('hex').slice(0, 8);
}

// Validates the public options and fixes the file layout. Without rootDir the project root is the component's own
// directory, so the relative name is the base name and nothing depends on the working directory.
export function resolveOptions(source, options) {
  if (typeof source !== 'string') throw new TypeError('compileSfc: source must be a string');
  if (options === null || typeof options !== 'object') throw new TypeError('compileSfc: options must be an object');
  const { filename, id, target = 'native', rootDir, trustedHtml = false } = options;
  if (typeof filename !== 'string' || filename === '') throw new TypeError('compileSfc: options.filename must be a non-empty string');
  if (!TARGETS.has(target)) throw new TypeError(`compileSfc: target must be "native", "web" or "test", got ${String(target)}`);
  if (id !== undefined && (typeof id !== 'string' || !ID_RE.test(id))) {
    throw new TypeError('compileSfc: options.id must be a non-empty string of letters, digits, "_" or "-"');
  }
  if (typeof trustedHtml !== 'boolean') throw new TypeError('compileSfc: options.trustedHtml must be a boolean');
  if (trustedHtml && target !== 'web') throw new TypeError('compileSfc: trustedHtml is only allowed for target "web"');

  let rootAbs;
  let fileAbs;
  if (rootDir !== undefined) {
    if (typeof rootDir !== 'string' || rootDir === '') throw new TypeError('compileSfc: options.rootDir must be a non-empty string');
    rootAbs = path.resolve(rootDir);
    fileAbs = path.resolve(rootAbs, filename);
  } else {
    fileAbs = path.resolve(filename);
    rootAbs = path.dirname(fileAbs);
  }
  const rel = toPosix(path.relative(rootAbs, fileAbs));
  let rootReal;
  try {
    rootReal = fs.realpathSync(rootAbs);
  } catch {
    throw new Error(`compileSfc: project root does not exist: ${rootDir ?? rootAbs}`);
  }
  return {
    source,
    target,
    trustedHtml,
    rel,
    fileAbs,
    rootReal,
    shortId: id ?? defaultId(rel),
  };
}

// Real path of file when it exists, is a regular file, and lies inside rootReal (symlinks included); otherwise null.
export function realInsideRoot(file, rootReal) {
  let real;
  try {
    real = fs.realpathSync(file);
    if (!fs.statSync(real).isFile()) return null;
  } catch {
    return null;
  }
  return real === rootReal || real.startsWith(rootReal + path.sep) ? real : null;
}

// Diagnostic text layer: every file that can carry a diagnostic has its text and line map registered here.
function textEntry(text) {
  return { text, lines: createLineMap(text) };
}

export class SfcContext {
  constructor({ source, rel, fileAbs, rootReal, target, trustedHtml, shortId }) {
    this.source = source;
    this.rel = rel;
    this.fileAbs = fileAbs;
    this.rootReal = rootReal;
    this.target = target;
    this.trustedHtml = trustedHtml;
    this.shortId = shortId;
    this.scopeId = `data-v-${shortId}`;
    this.texts = new Map([[rel, textEntry(source)]]);
    this.diags = [];
    this.deps = new Set();
  }

  registerFile(file, text) {
    this.texts.set(file, textEntry(text));
  }

  // Adds a diagnostic for [start, end) of file. Offsets are clamped to the file text, so a wrong offset from a helper
  // still produces a valid position instead of a crash.
  report(code, file, start, end, extra = {}) {
    const entry = this.texts.get(file);
    if (!entry) throw new Error(`SfcContext: no text registered for ${file}`);
    const len = entry.text.length;
    const s = Math.min(Math.max(0, start), len);
    const e = Math.min(Math.max(s, end), len);
    this.diags.push(createAnyDiagnostic(code, {
      message: extra.message,
      hint: extra.hint,
      severity: extra.severity ?? 'error',
      file,
      start: entry.lines.posAt(s),
      end: entry.lines.posAt(e),
    }));
  }

  // Span of a block's opening tag in the component file: from "<" to the end of the ">" that precedes its content.
  blockSpan(block) {
    const contentStart = block.loc.start.offset;
    const tagStart = Math.max(0, this.source.lastIndexOf('<', contentStart - 1));
    return [tagStart, contentStart];
  }

  // Dependencies are project-relative paths; a file outside the root never reaches this set.
  addDependency(realFile) {
    this.deps.add(toPosix(path.relative(this.rootReal, realFile)));
  }

  // The official compiler names files relative to the project root (its descriptor filename is root-relative), so a
  // relative name is anchored at the root, never at the working directory. Absolute names are used as they are.
  rootPath(file) {
    return path.isAbsolute(file) ? file : path.join(this.rootReal, file);
  }

  // Root-limited file access for the official compiler (type resolution of imported types). Refused files look
  // missing, so the compiler reports its own error and the frontend turns it into a diagnostic.
  // `record: false` reads without making the file a dependency; macro-types.mjs uses it for its probes and records the
  // type files it reads itself, because compiler-sfc's file cache hides the reads from a later compile in one process.
  scriptFs({ record = true } = {}) {
    const inside = (file) => realInsideRoot(this.rootPath(file), this.rootReal);
    return {
      fileExists: (file) => inside(file) !== null,
      readFile: (file) => {
        const real = inside(file);
        if (!real) return undefined;
        if (record) this.addDependency(real);
        return fs.readFileSync(real, 'utf8');
      },
      realpath: (file) => inside(file) ?? this.rootPath(file),
    };
  }

  // The project file a dependency name from the official compiler stands for, or null when it is not a file in the root.
  realDependency(dep) {
    return realInsideRoot(this.rootPath(dep), this.rootReal);
  }

  // Project-relative file that a relative type import names, or null. Mirrors the file lookup of @vue/compiler-sfc 3.5.43
  // (resolveExt): the specifier with a .js, .mjs or .cjs extension dropped, then .ts, .tsx and .d.ts (for .mjs first .mts
  // and .d.mts, for .cjs first .cts and .d.cts), then the index files of a directory. Files outside the root never match.
  resolveTypeImport(spec) {
    const joined = path.posix.join(path.posix.dirname(this.rel), spec);
    const moduleType = joined.endsWith('.mjs') ? 'm' : joined.endsWith('.cjs') ? 'c' : 'u';
    const base = joined.replace(/\.[cm]?jsx?$/, '');
    const ts = [`${base}.ts`, `${base}.tsx`, `${base}.d.ts`];
    const mts = [`${base}.mts`, `${base}.d.mts`];
    const cts = [`${base}.cts`, `${base}.d.cts`];
    const candidates = [
      base,
      ...(moduleType === 'm' ? [...mts, ...ts] : moduleType === 'c' ? [...cts, ...ts] : [...ts, ...mts, ...cts]),
      `${base}/index.ts`, `${base}/index.tsx`, `${base}/index.d.ts`,
    ];
    const fileExists = this.scriptFs().fileExists;
    const hit = candidates.find((file) => fileExists(file));
    return hit === undefined ? null : toPosix(path.posix.normalize(hit));
  }

  // Reads a block src attribute relative to the component directory. Returns { real, rel, text } or null after
  // reporting VN6008 at the block tag.
  readBlockFile(block, src) {
    const abs = path.resolve(path.dirname(this.fileAbs), src);
    const real = realInsideRoot(abs, this.rootReal);
    if (!real) {
      const exists = fs.existsSync(abs);
      const message = exists
        ? `src "${src}" resolves outside the project root`
        : `src "${src}" does not name a readable file`;
      const [s, e] = this.blockSpan(block);
      this.report('VN6008', this.rel, s, e, { message });
      return null;
    }
    this.addDependency(real);
    const rel = toPosix(path.relative(this.rootReal, real));
    const text = fs.readFileSync(real, 'utf8');
    this.registerFile(rel, text);
    return { real, rel, text };
  }
}

// Manifest helpers. Lookups are default-deny: an unknown name has no status and is not supported.
export function manifestEntry(kind, name) {
  return lookup(kind, name);
}

export function vueSupported(name, target) {
  return isSupported('vue', name, target === 'test' ? 'native' : target, { origin: 'user' });
}

export function vueCompilerSupported(name, target) {
  return isSupported('vue', name, target === 'test' ? 'native' : target, { origin: 'compiler' });
}

export function isNodeBuiltin(spec) {
  return spec.startsWith('node:') || NODE_BUILTINS.has(spec);
}
