// vue-native project access (WP-APP1). A project is a directory; compileProject names its modules by root-relative ids
// ('/src/App.vue'). A directory argument is the project root and its entry is src/App.vue. A file argument is the entry:
// its directory is the root unless --root names another one.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { compileProject } from '../../compiler/project/index.mjs';
import { formatDiagnostic } from '../../compiler/diagnostics/format.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
// The framework checkout (crates/, hosts/, tooling/): VN_FRAMEWORK_ROOT when set, else the repository containing this file.
export const FRAMEWORK_ROOT = path.resolve(process.env.VN_FRAMEWORK_ROOT ?? path.join(here, '..', '..'));
export const DEFAULT_ENTRY = 'src/App.vue';

// A mistake in how the CLI was called (exit status 2), as opposed to a project that fails to compile (exit status 1).
export class UsageError extends Error {
  constructor(message) {
    super(message);
    this.name = 'UsageError';
  }
}

// { root, entry }: an absolute root directory and the root-relative entry path ('src/App.vue').
export function resolveProject(input, { root } = {}) {
  const abs = path.resolve(input);
  if (!fs.existsSync(abs)) throw new UsageError(`no such file or directory: ${input}`);
  if (fs.statSync(abs).isDirectory()) {
    if (!fs.existsSync(path.join(abs, DEFAULT_ENTRY))) {
      throw new UsageError(`${input} has no ${DEFAULT_ENTRY}; pass the entry .vue file instead`);
    }
    return { root: abs, entry: DEFAULT_ENTRY };
  }
  const base = root === undefined ? path.dirname(abs) : path.resolve(root);
  const rel = path.relative(base, abs).split(path.sep).join('/');
  if (rel === '..' || rel.startsWith('../') || path.isAbsolute(rel)) {
    throw new UsageError(`entry ${input} is outside the project root ${base}`);
  }
  return { root: base, entry: rel };
}

// Reads one root-relative module id from disk. null when there is no regular file there.
export function diskReader(root) {
  return (id) => {
    const file = path.join(root, id);
    try {
      return fs.statSync(file).isFile() ? fs.readFileSync(file, 'utf8') : null;
    } catch {
      return null;
    }
  };
}

// Compiles the project on disk. Returns the compileProject result plus `sourceOf(file)` for diagnostic rendering.
export function compileAt({ root, entry, target = 'web' }) {
  const read = diskReader(root);
  const texts = new Map();
  const result = compileProject({
    entry,
    resolver: (id) => {
      const text = read(id);
      if (text !== null) texts.set(id.slice(1), text);
      return text;
    },
    target,
    sfcRootDir: root,
  });
  const sourceOf = (file) => {
    if (texts.has(file)) return texts.get(file);
    const text = read(`/${file}`);
    return text ?? '';
  };
  return { ...result, sourceOf };
}

// Human-readable diagnostics, one block per diagnostic (compiler/diagnostics/format.mjs). The compiler lowers the script
// of a .vue module from generated code, and its VN9001 spans point into that code, not into the file. Those are printed
// as a position only (no excerpt is invented), as are spans past the end of the file.
export function formatDiagnostics(diagnostics, sourceOf, { color = false } = {}) {
  return diagnostics.map((d) => {
    const source = sourceOf(d.file);
    const generated = d.file.endsWith('.vue') && d.code === 'VN9001';
    if (!generated && d.start.offset <= source.length) return formatDiagnostic(d, source, { color });
    const hint = d.hint ? `\n  = hint: ${d.hint}` : '';
    return `${d.file}:${d.start.line}:${d.start.column}: ${d.severity} ${d.code}: ${d.message}\n`
      + `  note: this position is in generated code, not in ${d.file}${hint}`;
  }).join('\n\n');
}

// The JSON form of one diagnostic (--json output).
export function diagnosticJson(d) {
  return {
    code: d.code,
    severity: d.severity,
    message: d.message,
    hint: d.hint,
    file: d.file,
    start: { offset: d.start.offset, line: d.start.line, column: d.start.column },
    end: { offset: d.end.offset, line: d.end.line, column: d.end.column },
  };
}
