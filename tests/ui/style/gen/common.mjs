// Shared helpers for the WP-U1b oracle corpora (crates/ui). Every expected value is produced by headless Chrome
// through tooling/chrome (WP-X1), never by the Rust code. Deterministic: a fixed-seed xorshift64* PRNG, no clock or
// environment input, and stable JSON key order, so a rerun rewrites byte-identical files.
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

export const CORPORA_DIR = join(dirname(fileURLToPath(import.meta.url)), '..', 'corpora');

if (Number(process.versions.node.split('.')[0]) < 22) {
  throw new Error(`node >= 22 required, got ${process.version}`);
}

const M64 = (1n << 64n) - 1n;

// xorshift64* with a per-generator seed. BigInt keeps the 64-bit arithmetic exact.
export function makeRng(seed) {
  let s = BigInt(seed) & M64;
  if (s === 0n) s = 1n;
  const next64 = () => {
    s ^= s >> 12n;
    s ^= (s << 25n) & M64;
    s ^= s >> 27n;
    return (s * 0x2545f4914f6cdd1dn) & M64;
  };
  const int = (n) => Number(next64() % BigInt(n)); // 0 <= r < n
  return {
    int,
    pick: (a) => a[int(a.length)],
    chance: (p) => int(1000000) < p * 1000000,
    range: (lo, hi) => lo + int(hi - lo + 1), // inclusive
  };
}

// Writes a corpus as JSON with a trailing newline. `outDir` defaults to tests/ui/style/corpora.
export function writeCorpus(name, value, outDir = CORPORA_DIR) {
  mkdirSync(outDir, { recursive: true });
  const file = join(outDir, name);
  writeFileSync(file, JSON.stringify(value, null, 1) + '\n');
  return file;
}
