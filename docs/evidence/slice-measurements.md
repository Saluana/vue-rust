# Slice measurements (task 4.5) — 2026-10-10

Measured on branch `claude/w45-measure` before it was merged. Measured in this container only: 4 vCPU, shared with other jobs. These are the startup, memory and edit-latency measurements that task 4.5 lists as missing for the Counter and Quick Quote slice apps. They do not qualify any device, and the G3 decision and the progress ledger stay with the maintainer.

## What was built

- `tooling/cli/headless/main.rs`: an optional `--timing` flag, documented in the file header. It writes `vn-timing` JSON lines to stderr: one mount line, one line per click or input step, and one exit line. The stdout result is unchanged. Without the flag the runner prints nothing extra and runs no collection. With the flag it runs a full collection after the mount and after each step, outside the timed windows, as the web host does at the end of each event. The app is now rooted for the run, as the web host roots it.
- `tooling/report/slice-measure.mjs`: builds both apps once as release headless binaries (through `tooling/cli/build.mjs`), runs them, checks their results, and prints a table, or one JSON document with `--json` (summaries and raw nanosecond samples). Options: `--startup-runs` (default 100), `--edit-runs` (default 5), `--iterations` (default 200).
- `tests/cli/headless.test.mjs`: one new test in the Counter block. `--timing` leaves stdout unchanged, and the default run writes no `vn-timing` line.
- `tests/tooling/report/slice-measure.test.mjs`: 22 tests of the percentile and summary helpers, the timing-line parser, the workload definitions and the state checks. Expected values are worked out by hand in the file. No test asserts a timing.

## Machine and conditions

- Container: Intel Xeon @ 2.80 GHz, 4 vCPU, 15.7 GiB RAM, Linux 6.18.44-fc-v114, shared with other jobs. Node v22.22.0, rustc 1.99.0. The generated crates use the headless release profile (LTO, one codegen unit).
- The 1-minute load average at the start of the three full runs was 3.52, 5.52 and 2.62 (the tool prints it for each run). Other jobs ran in the same container during every run, so the numbers carry that noise. The variance table below shows its size.

## What each figure is

- **main() to mounted** (`--timing` runs): an in-process `Instant` taken at the first statement of `main`, read after the mount, the drained scheduler flush and the error check. It excludes process exec, dynamic loading and everything before `main`.
- **spawn to exit**: Node `hrtime` around `spawnSync`, for the whole process. Reported for a default run (no `--timing`) and for a `--timing` run, which also performs the extra collections. Spawn to "first render complete" is not measured as such, because the runner cannot timestamp it from outside the process. Spawn to exit bounds it from above, and main() to mounted measures the in-process part.
- **Edit latency**: for each scripted click or input, from just before its first host operation (setting the value, or dispatching the click) to the end of `settle`, which runs up to four rounds of nextTick callbacks and `run_jobs`. The scheduler flushes as a promise job and `run_jobs` drains the queue, so the window ends when the queue is empty. The runner checks that no job runs after the window and fails the run if one does. Headless: no layout, style, paint, compositor, input device or browser event path. This is a lower bound of the framework work, not a user-perceived latency.
- **collect after each event**: a full mark-sweep after the window closes. It is not part of the edit latency. The web host runs the same collection at the end of each event, so a user would experience it as part of the event.
- **Memory**: `VmHWM` (peak resident set) and `VmRSS` from `/proc/self/status` at the exit line (Linux only). Heap: `Rt::heap_stats()` after a full collection, giving `live_objects` and `bytes_estimate`, the runtime's own per-object estimate and not allocator usage.
- **Percentiles**: nearest rank, meaning the sample at rank ceil(p·n/100). Every reported value is a measured sample. n is shown. Startup has 100 rounds per app, so p99 is the second-largest sample. Edit samples are pooled over 5 process runs per app, 200 iterations each.
- **Workloads**: a Quick Quote iteration has 20 timed events: 18 inputs (12 prefixes of "Ada Lovelace" typed into the customer field, a clear, three quantity edits, two price edits) and 2 clicks (hide and show the form). It covers the operations of the Quick Quote interaction test in `tests/cli/headless.test.mjs`, rearranged so that it can repeat. That test's script is not reused, because it removes line 1 and could not run twice. A Counter iteration has 7 clicks. Each iteration ends in the state it started in. The final snapshot of every run is checked against the hand-computed state. Quick Quote: total $29.99 (2 × 1250 + 1 × 499 = 2999 cents), two lines, form shown, no error line. Counter: count 0, double 0, reset disabled, no threshold message.

## Commands and results

Run from `/home/claude/wt/w45-measure` with `CARGO_TARGET_DIR=/home/claude/targets/w45-measure`.

| Command | Result |
|---|---|
| `node --test "tests/cli/*.test.mjs"`, before any change (baseline) | 42 tests: 42 pass, 0 fail, 0 skipped (325 s) |
| `node --test "tests/cli/*.test.mjs"`, after the change | 43 tests: 43 pass, 0 fail, 0 skipped (282 s) |
| `node --test "tests/tooling/report/*.test.mjs"` | 22 tests: 22 pass, 0 fail, 0 skipped |
| `node tooling/report/slice-measure.mjs` (acceptance, final code) | exit 0; 8 of 8 checks ok (output below) |
| `node tooling/report/slice-measure.mjs`, two earlier full runs (code identical except comment text in `main.rs`) | exit 0 each; 8 of 8 checks ok each (variance table below) |
| `node tooling/report/slice-measure.mjs --json --startup-runs 1 --edit-runs 1 --iterations N`, N = 50, 200, 400 | exit 0 each; all checks ok (growth table below) |
| `node tooling/cli/vue-native.mjs build examples/counter --target headless --out <dir>`, then the binary with and without `--timing` | stdout byte-identical (`cmp`); no stderr without the flag; with the flag, one mounted line, one line per click, one exit line |
| `cargo build --release --manifest-path <counter .vn-crate>/Cargo.toml`, forced rebuild | builds. One warning, in the generated `src/vn_async.rs` (unused imports), which this change does not touch. None from `main.rs` |

An earlier small run (`--startup-runs 3 --edit-runs 2 --iterations 3`) failed two Counter checks, and the failure is kept in this note. The state check did not skip Vue's scope stamps (`data-v-<hash>=""`), which the runner prints before the other attributes of a tag. The disabled reset button was therefore not found. The check now strips the stamps, as `tests/cli/headless.test.mjs` does, and a unit test covers the stamped form. The tool exits 1 whenever any check fails.

## Output of the acceptance run

`node tooling/report/slice-measure.mjs`, stdout, exit 0. Its stderr holds only the six progress lines (building, startup, edit latency for each app).

```
vue-native slice measurements (task 4.5): RELEASE headless binaries, run in this container
Headless: no layout, paint, compositor or input device. Edit latency is a lower bound of the framework work.
machine: Intel(R) Xeon(R) Processor @ 2.80GHz, 4 vCPU, 15.7 GiB RAM, Linux 6.18.44-fc-v114, node v22.22.0, rustc 1.99.0 (b940084d7 2026-09-28)
load average (1, 5, 15 min): start 2.62 3.95 4.06; end 2.84 3.65 3.94. The container is shared with other jobs.

Startup, 100 rounds per app, each a default run and a --timing run (microseconds).
main() to mounted: in-process Instant from the first statement of main, in a --timing run. spawn to exit: Node wall time
of the whole process, in a default run and in a --timing run.
app          metric                          n       min        p50        p90        p99        max
counter      main() to mounted (--timing)  100  1247.235   1390.305   2067.189   3031.682   3476.851
counter      spawn to exit (default run)   100  6922.481   8633.521  10119.551  11084.638  12615.007
counter      spawn to exit (--timing run)  100  7255.187   9163.376  10800.259  13147.498  20293.157
quick-quote  main() to mounted (--timing)  100  2982.464   3391.585   5155.015   8815.719  14075.940
quick-quote  spawn to exit (default run)   100  8295.330  11640.386  17270.295  28038.350  29052.141
quick-quote  spawn to exit (--timing run)  100  8456.279  12182.977  17025.860  19507.398  23732.927

Memory. Peak RSS is VmHWM from /proc/self/status at exit (Linux only), in KiB. Heap figures are the vn-value
stats after a full collection: live objects, and bytes_estimate (the runtime's per-object estimate, not allocator usage).
app          metric                                    n     min     p50     p90     p99     max
counter      peak RSS (VmHWM) at exit, startup runs  100    5072    5224    5292    5328    5336
counter      live objects after mount                100     796     796     796     796     796
counter      bytes_estimate after mount              100  155496  155496  155496  155496  155496
counter      peak RSS (VmHWM) at exit, edit runs       5    7008    7136    7152    7152    7152
counter      live objects after script, edit runs      5     797     797     797     797     797
counter      bytes_estimate after script, edit runs    5  155688  155688  155688  155688  155688
quick-quote  peak RSS (VmHWM) at exit, startup runs  100    5996    6104    6196    6220    6228
quick-quote  live objects after mount                100    1628    1628    1628    1628    1628
quick-quote  bytes_estimate after mount              100  273480  273480  273480  273480  273480
quick-quote  peak RSS (VmHWM) at exit, edit runs       5   13052   13128   13204   13204   13204
quick-quote  live objects after script, edit runs      5    1628    1628    1628    1628    1628
quick-quote  bytes_estimate after script, edit runs    5  273576  273576  273576  273576  273576

Edit latency (microseconds). Dispatch of one scripted click or input to the end of the scheduler flush that
patches the DOM (the job queue is empty). 5 process runs per app, 200 iterations each,
samples pooled over the runs. "collect" is the full collection after each event, outside that window, which the web host also runs.
app          metric                        n      min       p50       p90       p99       max
counter      click (all)                7000   48.280    64.588    88.583   151.676  2194.733
counter        click #increment         6000   48.280    63.310    88.527   154.664  2194.733
counter        click #reset             1000   54.925    67.612    89.644   137.340   704.664
counter      collect after each event   7000   17.398    23.180    30.838    66.335  1741.338
quick-quote  click (all)                2000  145.322  1364.067  1839.088  2877.509  5073.293
quick-quote    click #toggle            2000  145.322  1364.067  1839.088  2877.509  5073.293
quick-quote  input (all)               18000  170.287   233.544   492.261   783.347  5287.639
quick-quote    input #customer         13000  170.287   211.383   293.584   489.220  5287.639
quick-quote    input #quantity-2        3000  337.159   480.115   633.122  1018.349  2713.607
quick-quote    input #price-1           2000  333.220   427.544   562.217   887.713  2613.328
quick-quote  collect after each event  20000   65.825    88.829   143.679   322.977  3835.871

Checks
  [ok] counter: the state check rejects a wrong state (negative control): #count: expected "Count: 0", got "Count: 1"
  [ok] counter: every edit run prints the result of a plain run
  [ok] counter: every edit run times every click and input
  [ok] counter: initial and final states match the hand-computed state
  [ok] quick-quote: the state check rejects a wrong state (negative control): #total: expected "Total: $29.99", got "Total: $39.97"
  [ok] quick-quote: every edit run prints the result of a plain run
  [ok] quick-quote: every edit run times every click and input
  [ok] quick-quote: initial and final states match the hand-computed state
```

## Run-to-run variation

Three full runs with the default settings, on the same container. Values are microseconds unless marked KiB, shown as p50 / p99 of the pooled samples.

| Row | Run 1 | Run 2 | Run 3 (output above) |
|---|---|---|---|
| counter, main() to mounted | 1314.202 / 5470.828 | 1324.167 / 2444.390 | 1390.305 / 3031.682 |
| counter, spawn to exit, default run | 7691.698 / 27113.510 | 6638.138 / 11220.174 | 8633.521 / 11084.638 |
| quick-quote, main() to mounted | 3638.894 / 7820.038 | 3292.078 / 6931.311 | 3391.585 / 8815.719 |
| quick-quote, spawn to exit, default run | 11363.746 / 16711.831 | 10214.192 / 14883.167 | 11640.386 / 28038.350 |
| counter, click #increment | 63.243 / 173.505 | 64.836 / 172.356 | 63.310 / 154.664 |
| quick-quote, click #toggle | 724.698 / 2940.983 | 1376.984 / 2826.631 | 1364.067 / 2877.509 |
| quick-quote, input (all) | 239.428 / 792.731 | 226.835 / 677.463 | 233.544 / 783.347 |
| quick-quote, input into #customer | 213.626 / 435.667 | 209.250 / 395.084 | 211.383 / 489.220 |
| quick-quote, collect after each event | 90.099 / 339.061 | 87.194 / 301.565 | 88.829 / 322.977 |
| quick-quote, peak RSS at startup (KiB) | 6108 | 6100 | 6104 |
| quick-quote, peak RSS after 200 iterations (KiB) | 13112 | 13140 | 13128 |
| counter, peak RSS at startup (KiB) | 5224 | 5224 | 5224 |
| counter, peak RSS after 200 iterations (KiB) | 7124 | 7084 | 7136 |

The toggle-click p50 in run 1 is about half of runs 2 and 3. Its cause is not established, and the load averages do not line up with it. The other rows agree closely across runs, apart from the tails (p99 and max), which move the most.

## Memory growth with the number of events

Final code, one edit run per setting (`--startup-runs 1 --edit-runs 1`).

| Iterations | Quick Quote events | Quick Quote peak RSS (KiB) | Quick Quote live objects after script | Counter events | Counter peak RSS (KiB) | Counter live objects after script |
|---|---|---|---|---|---|---|
| 50 | 1000 | 7904 | 1628 | 350 | 5680 | 797 |
| 200 | 4000 | 13072 | 1628 | 1400 | 7152 | 797 |
| 400 | 8000 | 19900 | 1628 | 2800 | 8952 | 797 |

Between 50 and 400 iterations, peak RSS rises by about 1.7 KiB per Quick Quote event ((19900 − 7904) / (8000 − 1000)) and about 1.3 KiB per Counter event ((8952 − 5680) / (2800 − 350)). Live vn-value objects and `bytes_estimate` do not change with the workload (Quick Quote 1628 objects and 273576 bytes after the script, against 1628 and 273480 after mount; Counter 797 and 155688, against 796 and 155496). So the peak RSS of an edit run is not a steady-state footprint. Part of the growth is the runner itself, which keeps every step's result until it prints the JSON array. The rest is not attributed. I did not check the DOM host's node storage or allocator retention.

## Negative controls and checks

- Helper tests: a wrong percentile expectation (the p50 of 1 to 10 expected as 6; nearest rank gives 5) fails. A wrong summary fails, and so does a wrong parsed `ns`. A malformed line, a missing `ns`, an invalid `op` and an unknown phase each throw rather than being skipped. A wrong total gives one mismatch. A leftover error line plus a missing editor give two. A wrong count gives one. A snapshot with scope stamps passes, and the same snapshot with the disabled attribute removed gives one mismatch.
- `tests/cli`: the `--timing` test asserts identical stdout and the phase sequence `mounted`, `step`, `exit`. The default run has no `vn-timing` line.
- The tool, on every run: each edit run's stdout is compared with the stdout of a plain run of the same script, and the number of timed events is checked. The final snapshot of each run is checked against the hand-computed state. A negative control changes one value in the real final snapshot of each app, and that must produce exactly one mismatch. Any failed check makes the tool exit 1.

## Limits

1. Headless only: no layout, style, paint, compositor, input device or browser. Edit latency is a lower bound of the framework work, not what a user perceives.
2. One shared 4-vCPU container, not a named device. The run-to-run spread above is large, and the tails move the most. Each max is a single sample. Nothing here characterises a device.
3. Startup "main() to mounted" excludes exec, dynamic loading and everything before `main`. Spawn to exit includes them, plus Node's `spawnSync` overhead.
4. A `--timing` run collects after every step, so its memory is not the default run's memory. The equality check covers the results only. Peak RSS was measured only in `--timing` runs.
5. Edit samples are pooled over 5 processes per app. The first iteration of each process, with cold caches, is included and not separated out.
6. The per-event RSS growth is not attributed (see above), so edit-run peak RSS is not a steady-state footprint.
7. `bytes_estimate` is the runtime's estimate, not allocator usage.
8. No browser (Chrome) measurements. Chrome cannot run in this container, so the browser tests were not run here.
9. Artifact size is not measured here; task 3.1 covers it.
10. With 100 startup rounds, p99 is the second-largest sample, so read it with that in mind.

## Open questions

1. Should the post-event collection count as edit latency? The web host runs it inside the event. Here it is reported separately.
2. Where does the per-event RSS growth come from? A next step would be to count live DOM-host nodes and to measure the result buffer's share.
3. Is nearest rank the percentile convention the maintainer wants, or should interpolation be used?

## Files

- `tooling/cli/headless/main.rs` (changed)
- `tests/cli/headless.test.mjs` (changed)
- `tooling/report/slice-measure.mjs` (new)
- `tests/tooling/report/slice-measure.test.mjs` (new)
- `docs/evidence/slice-measurements.md` (this note)

Nothing was committed or pushed. `planning/**`, `docs/dev/tasks-progress.json` and `docs/dev/STATUS.md` were not edited.
