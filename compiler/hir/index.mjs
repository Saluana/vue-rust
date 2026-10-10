// HIR entry point (WP-C1b). lowerToHir(ast, analysis, options) -> { hir, diagnostics }. hir is null when lowering
// stopped at a construct it does not handle (the diagnostic says where); the caller produces no Rust then.
import { makeContext, LowerAbort, HIR_VERSION } from './context.mjs';
import { lowerModuleAst } from './module.mjs';
import { identityLocator } from '../sourcemap.mjs';

export { HIR_VERSION };

// locate: offset of source -> the author's position (compiler/sourcemap.mjs); source itself when omitted.
export function lowerToHir(ast, analysis, { source, filename = '<input>', specifier, origin = 'user', lang = 'js', resolve, locate } = {}) {
  if (typeof source !== 'string') throw new TypeError('lowerToHir: options.source must be the source string');
  if (typeof specifier !== 'string' || specifier === '') throw new TypeError('lowerToHir: options.specifier is required');
  const cx = makeContext(ast, analysis, { source, filename });
  cx.resolve = resolve ?? ((s) => s);
  cx.locate = locate ?? identityLocator(source, filename);
  try {
    const hir = lowerModuleAst(cx, { specifier, origin, lang });
    return { hir, diagnostics: [] };
  } catch (e) {
    if (e instanceof LowerAbort) return { hir: null, diagnostics: [e.diagnostic] };
    throw e;
  }
}
