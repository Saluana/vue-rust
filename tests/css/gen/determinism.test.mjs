// WP-U1a: the corpus generators are deterministic (two runs give identical bytes) and reproduce the committed
// corpora in tests/css/corpora. Each run drives headless Chrome through tooling/chrome; without Chrome the tests
// are reported as skipped with the reason (never silently passed).
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { chromeSkipReason } from '../../../tooling/chrome/chrome.mjs';
import { CORPORA_DIR } from './common.mjs';
import { generate as selectorCorpus } from './gen-selector-corpus.mjs';
import { generate as mediaCorpus } from './gen-media-corpus.mjs';
import { generate as cascadeCorpus } from './gen-cascade-corpus.mjs';

const skip = chromeSkipReason() ?? false;

// Runs a generator twice into scratch directories and checks each output against the other run and the committed file.
async function checkTwice(generate, names) {
  const dir = mkdtempSync(join(tmpdir(), 'vn-css-corpus-'));
  try {
    const first = [].concat(await generate(join(dir, 'a')));
    const second = [].concat(await generate(join(dir, 'b')));
    assert.equal(first.length, names.length);
    for (let i = 0; i < names.length; i++) {
      const a = readFileSync(first[i]);
      const b = readFileSync(second[i]);
      assert.ok(a.equals(b), `${names[i]} differs between two runs`);
      const committed = readFileSync(join(CORPORA_DIR, names[i]));
      assert.ok(a.equals(committed), `${names[i]} differs from the committed corpus; rerun the generator and commit it`);
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test('selector corpus is deterministic and matches the committed file', { skip, timeout: 120000 }, async () => {
  await checkTwice(selectorCorpus, ['selectors.json']);
});

test('media corpus is deterministic and matches the committed file', { skip, timeout: 120000 }, async () => {
  await checkTwice(mediaCorpus, ['media.json']);
});

test('cascade and serialisation corpora are deterministic and match the committed files', { skip, timeout: 240000 }, async () => {
  await checkTwice(cascadeCorpus, ['cascade.json', 'serialize.json']);
});
