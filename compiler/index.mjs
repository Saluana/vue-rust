// compileModule (WP-C1b): JavaScript/TypeScript module source -> Rust source. Stages: parse, scope analysis and profile
// check (compiler/frontend), HIR lowering (compiler/hir), emission (compiler/emit). Any error diagnostic stops the
// pipeline and no Rust is produced. Warnings are returned with the result.
import { checkSource } from './frontend/check.mjs';
import { lowerToHir, HIR_VERSION } from './hir/index.mjs';
import { emitModule } from './emit/index.mjs';

export { HIR_VERSION };
export { compileProject } from './project/index.mjs';

// compileModule(source, { filename, lang, specifier, origin, target, resolve, locate })
// -> { rust, hir, diagnostics, requests }. rust is null when diagnostics contain an error. locate maps an offset of source
// to the author's position (compiler/sourcemap.mjs); by default source is its own position.
export function compileModule(source, options = {}) {
  const { filename = '<input>', lang = 'js', specifier, origin = 'user', target = 'native', resolve, locate } = options;
  if (typeof specifier !== 'string' || specifier === '') throw new TypeError('compileModule: options.specifier is required');
  const { ast, analysis, diagnostics } = checkSource(source, { filename, lang, target, stdlib: true });
  if (!ast || diagnostics.some((d) => d.severity === 'error')) {
    return { rust: null, hir: null, diagnostics, requests: [] };
  }
  const lowered = lowerToHir(ast, analysis, { source, filename, specifier, origin, lang, resolve, locate });
  if (lowered.hir === null) return { rust: null, hir: null, diagnostics: [...diagnostics, ...lowered.diagnostics], requests: [] };
  const rust = emitModule(lowered.hir);
  return { rust, hir: lowered.hir, diagnostics, requests: lowered.hir.requests };
}
