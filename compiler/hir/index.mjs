// HIR entry point (WP-C1b). lowerToHir(ast, analysis, options) -> { hir, diagnostics }. hir is null when lowering
// stopped at a construct it does not handle (the diagnostic says where); the caller produces no Rust then.
import { makeContext, LowerAbort, HIR_VERSION } from './context.mjs';
import { lowerModuleAst } from './module.mjs';

export { HIR_VERSION };

export function lowerToHir(ast, analysis, { source, filename = '<input>', specifier, origin = 'user', lang = 'js', resolve } = {}) {
  if (typeof source !== 'string') throw new TypeError('lowerToHir: options.source must be the source string');
  if (typeof specifier !== 'string' || specifier === '') throw new TypeError('lowerToHir: options.specifier is required');
  const cx = makeContext(ast, analysis, { source, filename });
  cx.resolve = resolve ?? ((s) => s);
  try {
    const hir = lowerModuleAst(cx, { specifier, origin, lang });
    return { hir, diagnostics: [] };
  } catch (e) {
    if (e instanceof LowerAbort) return { hir: null, diagnostics: [e.diagnostic] };
    throw e;
  }
}
