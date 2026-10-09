// Shared constants and helpers for the compatibility-manifest tooling (WP-G0B).
import { createHash } from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
export const DESIGN_REL = 'planning/vue-native-framework/design.md';
export const PROFILE_REL = 'compiler/profile';
export const COMPAT_REL = 'docs/compatibility';
export const VUE_VERSION = '3.5.43';

// P faithful, A adaptation, W web-only, X excluded (error), E experimental (not promised).
export const STATUSES = ['P', 'A', 'W', 'X', 'E'];

// design.md sections 8.1-8.5 and the row-ID letter each one owns.
export const SECTION_LETTER = { '8.1': 'L', '8.2': 'S', '8.3': 'V', '8.4': 'U', '8.5': 'A' };
export const EXPECTED_COUNTS = { L: 21, S: 25, V: 29, U: 32, A: 19 };

export function expectedIds() {
  const ids = [];
  for (const [letter, count] of Object.entries(EXPECTED_COUNTS)) {
    for (let n = 1; n <= count; n++) ids.push(letter + String(n).padStart(2, '0'));
  }
  return ids;
}

export const cmp = (a, b) => (a < b ? -1 : a > b ? 1 : 0); // codepoint order, locale independent
export const sha256 = (text) => createHash('sha256').update(text).digest('hex');
export const toJson = (value) => JSON.stringify(value, null, 2) + '\n';
export const abs = (relPath) => path.join(ROOT, relPath);
