// Frontend pipeline used by tests and by the lowering WP: parse, scope analysis, profile check.
// Diagnostics from all stages are merged and sorted by source offset. When parsing fails there is no AST, so only the
// parse diagnostics are returned.
import { parseModule } from './parse.mjs';
import { analyzeScopes } from '../analysis/scope.mjs';
import { checkProfile } from '../analysis/profile-check.mjs';

export function checkSource(source, { filename = '<input>', lang = 'js', target = 'native', stdlib = false } = {}) {
  const parsed = parseModule(source, { filename, lang });
  if (!parsed.ast) return { ast: null, analysis: null, diagnostics: parsed.diagnostics };
  const analysis = analyzeScopes(parsed.ast, { source, filename });
  const profile = checkProfile(parsed.ast, { source, filename, lang, target, analysis, stdlib });
  const diagnostics = [...profile, ...analysis.diagnostics].sort((a, b) =>
    a.start.offset - b.start.offset || a.end.offset - b.end.offset || (a.code < b.code ? -1 : a.code > b.code ? 1 : 0));
  return { ast: parsed.ast, analysis, diagnostics };
}
