# Release artifact size and allocation report — 2026-10-10

WP 3.1, Part A (ticket w31-size). Part C, the engine-backed comparison, is not done; the reason is in its own section below.

## What was built

- `tooling/report/aot-size.mjs` builds in release mode through the project's own paths: the browser host wasm with `buildWebDist({ only: 'shipped' })` from `tooling/web/build-wasm.mjs` (the variant without the selftest export), and Counter and Quick Quote with `node tooling/cli/vue-native.mjs build <dir> --target web` and `--target headless`. For each artifact it reports raw bytes and gzip bytes (level 9). For native binaries it also reports the stripped size (`strip --strip-all` of a copy) and its gzip bytes.
- Contributors: for each native binary, `nm --size-sort -C --print-size --defined-only`, counting file-backed symbols only (types T t D d R r V v W w G g), summed by the crate of the demangled name. This is an approximation: `cargo bloat` is not installed here, and it was not installed for this report.
- Allocation: `tooling/cli/headless/main.rs` takes `--stats`. With it, the runner roots the app, collects the heap after each script step (as the web host does at each event), and prints one JSON line on stderr with the vn-value heap counters after mount and at the end. The results on stdout do not change; the report checks this. `crates/value/src/heap.rs` gained two high-water marks, `peak_live_objects` and `peak_bytes_estimate`, in `HeapStats`.
- For each example the report runs one interaction script twice, without and with `--stats`. It requires the expected values, which are written by hand from the demo specification, and identical stdout in both runs. If either fails, it writes no report and exits 1.
- Output: the text summary on stdout. The same summary is written as JSON to `$CARGO_TARGET_DIR/aot-size/summary.json` (schemaVersion 1). `--json` prints that JSON on stdout instead of the text.

## Commands and results

```
export CARGO_TARGET_DIR=/home/claude/targets/w31-size
node tooling/report/aot-size.mjs
```

Exit 0. Output, verbatim from the run that produced the numbers in this note:

```
release artifact sizes (rustc 1.99.0 (b940084d7 2026-09-28); node v22.22.0)
artifact                            raw bytes  gzip bytes   stripped  stripped gzip
web host wasm (vn_renderer_dom, shipped)      45988       15811          -              -
counter web app wasm                  1400570      461021          -              -
counter headless native (release)     3214840     1426734    2860880        1358760
quick-quote web app wasm              1511170      484898          -              -
quick-quote headless native (release)    3435912     1506455    3075976        1437780

approximate contributors of counter headless native (file-backed symbol bytes by crate, from nm; total 2328842 bytes in 2538 symbols)
  vn_runtime                            1034591  44.4%  634 symbols
  vn_value                               477327  20.5%  542 symbols
  vn_reactivity                          270654  11.6%  230 symbols
  core                                   177133   7.6%  552 symbols
  std                                     80615   3.5%  137 symbols
  vn_dom                                  74660   3.2%  98 symbols
  vn_app_counter                          51280   2.2%  25 symbols
  gimli                                   38394   1.6%  23 symbols
  hashbrown                               30812   1.3%  28 symbols
  alloc                                   28778   1.2%  164 symbols
  rustc_demangle                          16849   0.7%  35 symbols
  addr2line                               11631   0.5%  9 symbols

approximate contributors of quick-quote headless native (file-backed symbol bytes by crate, from nm; total 2514635 bytes in 2583 symbols)
  vn_runtime                            1034591  41.1%  634 symbols
  vn_value                               478115  19.0%  546 symbols
  vn_reactivity                          270654  10.8%  230 symbols
  vn_app_quick_quote                     236285   9.4%  66 symbols
  core                                   177133   7.0%  552 symbols
  std                                     80615   3.2%  137 symbols
  vn_dom                                  74660   3.0%  98 symbols
  gimli                                   38394   1.5%  23 symbols
  hashbrown                               30812   1.2%  28 symbols
  alloc                                   28778   1.1%  164 symbols
  rustc_demangle                          16849   0.7%  35 symbols
  addr2line                               11631   0.5%  9 symbols

heap of counter: passed (results match the demo specification; --stats does not change the results)
  realm creation and mount: 924 objects allocated, 924 live, 181344 bytes (estimate)
  interaction script (10 steps): 157 objects allocated, 10 collections, 284 freed in total
  at the end: 797 live objects, 155688 bytes (estimate); peak 950 live objects, 185784 bytes (estimate)

heap of quick-quote: passed (results match the demo specification; --stats does not change the results)
  realm creation and mount: 1835 objects allocated, 1835 live, 311592 bytes (estimate)
  interaction script (16 steps): 2022 objects allocated, 16 collections, 2229 freed in total
  at the end: 1628 live objects, 273576 bytes (estimate); peak 2070 live objects, 338256 bytes (estimate)

engine-backed comparison: not done. No published QuickJS or Boa size is cited in the repository docs, and no engine-backed build was made (WP 3.1 Part C). See docs/evidence/aot-size-report.md.
```

Environment: Linux x86_64 container, rustc 1.99.0, Node v22.22.0. Build profiles: the web apps use `opt-level "z"`, LTO, one codegen unit and `panic = "abort"` (`tooling/cli/build.mjs`); the headless binaries use LTO and one codegen unit (`tooling/cli/build.mjs`); the host wasm uses the workspace release profile (`Cargo.toml`: `debug = 0`, `codegen-units = 16`).

A second run with `--json` (same inputs and target directory, exit 0) printed JSON equal to `$CARGO_TARGET_DIR/aot-size/summary.json`. Its sizes and heap counters are the same as in the text run above, so the numbers reproduced across two runs.

## How to read the numbers

- Raw and gzip bytes of the web apps are the `bytes` and `gzipBytes` that the CLI writes to each `manifest.json`. The host wasm figures are the same fields from `buildWebDist`.
- Contributors are sums of symbol sizes. For counter the file-backed symbols add up to 2328842 bytes of a 3214840-byte file, so about 72 percent of the file is attributed to a crate. The rest is headers, section padding and data without symbol sizes. Generic instances are attributed to the crate named in their path, and LTO and inlining move code between crates. The per-crate figures are estimates: use them to compare the two example binaries with each other, not as an exact attribution.
- Heap counters are vn-value estimates (an object base plus property and element bytes), not allocator bytes. `allocated` counts objects that the runtime created since the realm was created. The `realm creation and mount` line is taken before any collection, so its live count still includes objects that are already garbage. The peaks are high-water marks since the realm was created; a collection does not lower them. The `interaction script` line is the end snapshot minus the afterMount snapshot, except that `freed in total` is cumulative since the realm was created.
- Interaction scripts. Counter has 10 steps: seven clicks and three snapshots. The expected values are the demo specification's: after five clicks Count is 5; after a sixth it is 6 and Double is 12; Reset gives Count 0. Quick Quote has 16 steps (inputs, clicks, reads of `#total` and `#status`, and snapshots). Its expected values are a total of $29.99 at the start; $39.97 after quantity 3 on line 2 (2 x 1250 + 3 x 499 = 3997 cents), with the status line naming the change; and $14.97 after line 1 is removed (3 x 499 = 1497 cents).

## Engine-backed comparison (Part C): not done

The comparison is not done, for two reasons. No published QuickJS or Boa build size is cited in the repository docs: `planning/vue-native-framework/design.md` names both engines in its alternatives table and links their documentation (bellard.org/quickjs and github.com/boa-dev/boa), but gives no size figure for either. And the ticket asks for no engine-backed build, so none was made. No figure has been taken from outside the repository and none has been estimated.

## Tests

- `node --test tests/tooling/report/aot-size.test.mjs` (the report's light tests; no release build): 10 tests, 10 pass, 0 fail. These cover the nm parser, the crate grouping and the symbol totals on hand-written fixtures, and the Quick Quote check. The check has a negative control: a total of $39.98 where $39.97 is expected is reported as a problem.
- `cargo test -p vn-value`: 706 passed, 0 failed, 0 ignored (28 unit tests and 678 in the integration test files of `crates/value/tests/`; doc-tests: 0). Includes `heap_stats_peaks_rise_on_allocation_and_survive_a_collection` in `crates/value/tests/kernel_gc.rs`, which allocates seven objects, checks that the peaks equal the live counts after allocation, and checks that a collection leaves the peaks in place.
- `node --test tests/cli/headless.test.mjs`: 14 tests, 14 pass, 0 fail, 0 skipped; exit 0 (output in the scratchpad at `w31/headless-tests.txt`). The headless runner changed, so the suite was run after the change. The report also runs each example with and without `--stats` and requires identical stdout; both examples passed that check in the run above.

## History of the first run

The first run of the script stopped with exit 1 at the Quick Quote check. The check compared whole tags, and Vue's scoped-style attribute (`data-v-<hash>`) on each element is not part of the text the demo specifies. The check now compares each element's text, as the headless tests do, and a unit test covers it. No number from the stopped run is used here.

## Limits

- The contributor lists are an approximation from `nm` symbol sizes, as described above. The shipped wasm files are not broken down by crate.
- Sizes were measured on this Linux container with rustc 1.99.0. They are not measurements for macOS, for any device, or for a shipped distribution, and they depend on the toolchain and profiles listed above.
- The web app wasm files were built and measured but not run. Chrome cannot run in this container, so no browser test was executed for this report.
- Allocation counts come from the headless runner, which collects after each step only in `--stats` mode. Without `--stats` no collection runs during a script, so the allocation figures describe the `--stats` run, which was checked to give the same results.
- One interaction script per example. The counters count vn-value heap objects. Rust-side allocations that are not heap objects, such as Rust strings and vectors, are not counted.
- The vn-value counters are new in this change; their numbers have not been compared with any other allocator measurement.

## Files

- `tooling/report/aot-size.mjs` (new)
- `tests/tooling/report/aot-size.test.mjs` (new)
- `tooling/cli/headless/main.rs` (`--stats`)
- `crates/value/src/heap.rs` (peak counters)
- `crates/value/tests/kernel_gc.rs` (peak test)
