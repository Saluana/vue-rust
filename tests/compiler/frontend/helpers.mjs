// Shared test helpers for the WP-C1a frontend tests. Not a test file (no .test.mjs suffix).
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
// Independent oracle: the project's own @babel/parser, called directly (not through compiler/frontend).
export const babel = require('@babel/parser');

// Parses with Babel's error recovery so duplicate-declaration errors reach the scope analysis. ts mode has no jsx plugin,
// matching the frontend: angle-bracket assertions parse as erased type assertions.
export function parseRecovering(source, { ts = false } = {}) {
  return babel.parse(source, {
    sourceType: 'module',
    errorRecovery: true,
    ranges: true,
    plugins: ts ? ['typescript'] : [],
  });
}

// "L:C-L:C" for a diagnostic span (1-based lines, 0-based columns, end exclusive).
export function spanText(d) {
  return `${d.start.line}:${d.start.column}-${d.end.line}:${d.end.column}`;
}

export function codesAndSpans(diags) {
  return diags.map((d) => `${d.code} ${spanText(d)}`);
}

// Strips class identity so a result can be compared with a JSON round trip.
export function plainResult(result) {
  return JSON.parse(JSON.stringify({ ...result, diagnostics: result.diagnostics.map((d) => ({ ...d })) }));
}
