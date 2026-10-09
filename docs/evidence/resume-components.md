# Component blocker repairs — 2026-10-09

Baseline HEAD: 05987fa. Baseline reverified: 1324 Rust passes / 2 known vn-ui cascade failures; language 79; lowering 166; Vue core 102; components 131 pass / 29 blocked; CLI 40 pass; browser host 293 pass / 105 documented skips. Task generator check passed.

Repairs: DOM host tags keep template refs raw; component-owned listeners use Vue error routing (including returned promise rejections); margin/padding expand to longhands and serialize/remove with priority-aware shorthands.

Validation (source scripts/env.sh; private CARGO_TARGET_DIR=target/resume-components):
- cargo test -p vn-dom -p vn-runtime -p vn-reactivity: 391 passed, 0 failed.
- node tooling/harness/run-language.mjs --with-vue --ui: 143 passed, 17 blocked, 0 failed, 0 unexpected after the explicit ledger update.
- node tooling/harness/run-language.mjs --with-vue: 102 passed.
- node tests/dom/gen/gen-dom-corpus.mjs: Chrome recorded 6 new box cases; before the repair all 6 failed on unsupported shorthand properties. Existing comparator negative controls remain passing.

The remaining 17 UI blockers are advanced components. Other shorthand families and functional box grammars are not qualified. The two vn-ui computed-style failures remain pre-existing.
