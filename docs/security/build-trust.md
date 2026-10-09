# Build trust: trusted and untrusted build contexts

Status: research-stage policy and tooling (WP-G0C, tasks 2.3; requirement R22.AC1; design section 13.1). The
sandbox described here is a best-effort research control. It is not a production security boundary.

## 1. What executes where

| Activity | Code that runs | Trust | Where it may run |
|---|---|---|---|
| Parsing and compiling `.vue` input with `@vue/compiler-sfc` and `@babel/parser` | Locked Vue and Babel packages, in-process in Node | Packages trusted after review. Parsing untrusted input is still parser code running on it | In the compiler. The input quotas proposed in design section 13.1 are not implemented by this work package |
| npm lifecycle scripts (`preinstall`, `install`, `postinstall`, `prepare`) | Any package's shell command | Untrusted unless reviewed | Disabled by default. Untrusted sandbox only, after review |
| `npm run` and `npm exec` of project scripts | Project scripts | Untrusted for a project the user has not reviewed | Untrusted sandbox |
| Cargo `build.rs` | Arbitrary Rust, compiled and run at build time with the user's privileges | Untrusted unless reviewed | Untrusted sandbox |
| Cargo procedural macros | Compiled code run inside `rustc` | Same as `build.rs` | Untrusted sandbox |
| `rustc` and the system linker (`cc`, `ld`) | Compiler and linker | `rustc` is the pinned toolchain (`rust-toolchain.toml`, 1.99.0). The system linker is not pinned | Trusted build |
| Generated application (native or WASM) at run time | Application code plus framework | As trusted as the application source. The framework's capability checks do not contain deliberately malicious compiled code (design section 13.1) | The user's session |
| Research fixtures with hostile input | Attacker-written code | Untrusted | Untrusted sandbox only |

## 2. Contexts

- Trusted build context: the framework repository at a reviewed commit, the pinned toolchain, and dependencies
  whose licence and integrity records are in `release/dependency-policy/`. A trusted build still runs with the
  developer's privileges, so keep credentials out of it where practical. Signing and publishing belong in a
  separate, isolated job that no untrusted code can reach.
- Untrusted build context: any project or package the user has not reviewed; every dependency script and
  `build.rs`; every hostile fixture. Artifacts from an untrusted run are data. They are not promoted into trusted
  output without being rebuilt in the trusted context.

## 3. Rules

1. Untrusted workers never see signing keys, release or registry tokens, cloud or SSH keys, npm or cargo
   credential files, keychain items, or the real home directory. `tooling/sandbox/run-untrusted.mjs` gives the
   child `HOME` and `TMPDIR` inside its workspace and passes only the `--env` values the caller names. The caller
   must not pass secrets.
2. Each experiment uses a disposable workspace. Never reuse a workspace that has run untrusted code for trusted
   work. Delete it afterwards.
3. Scripts are disabled by default: install with `npm ci --ignore-scripts`. A script is run only inside the
   sandbox, after review, and its text is recorded.
4. Untrusted workers have no network. Dependencies are fetched in a separate, trusted step restricted to the
   registry allowlist in `release/dependency-policy/POLICY.md`, and the fetched cache is then made readable (for
   example with `--read`) to the untrusted step.
5. Cargo is not a sandbox. `cargo build` runs `build.rs` and proc-macros with the user's full privileges. Building
   untrusted Rust therefore goes through the sandbox runner, or through an isolation layer whose evidence has been
   recorded.
6. Credential locations and system trees are refused as workspaces. The runner refuses any workspace or scratch
   directory that is or contains the home directory, the temp directory root, `/`, `/tmp`, or a system read tree
   (`/usr/lib`, `/System/Library`, `/bin`, `/usr/bin`). It also refuses any tree that overlaps `~/.ssh`, `~/.gnupg`,
   `~/.aws`, `~/.cargo`, `~/.config` or `~/Library/Keychains`. Read-only `--read` trees may be system paths.

## 4. The runner: `tooling/sandbox/run-untrusted.mjs`

Mechanism: `/usr/bin/sandbox-exec -p <profile>`, where the profile is generated and starts with `(deny default)`
and `(deny network*)`. Before any untrusted code runs, a self-test confirms that a read and a write of a file
outside every allowed tree both fail, and that the probe really executed. If either check fails, the runner exits
98 and runs nothing. If `sandbox-exec` is missing, or the binary is not named `sandbox-exec`, or the platform is
not macOS, the runner exits 97 and runs nothing. It never falls back to running unsandboxed. Other exit codes: the
child's status (128 + N if it was killed by signal N), 2 for usage errors, 70 for internal errors, and 124 for a
timeout.

What the generated profile allows (checked by `buildProfile` and by the tests):

- Reads: `/usr/lib`, `/System/Library`, `/bin`, `/usr/bin`, `/dev/random`, `/dev/urandom`, the command itself,
  any `--read` trees, and the workspace and scratch directories.
- Writes: the workspace and scratch directories, and `/dev/null`.
- Execution: `/bin`, `/usr/bin`, the workspace and scratch directories, and the command.
- Metadata (stat): the directories above allowed trees. Their contents stay unreadable.
- Directory listing: top-level names of `/` only, because Node reads `/` at startup.
- `sysctl` reads and `fork`: allowed.
- Network: denied entirely.

Everything else is denied by default. That includes other file reads and writes, system services (mach lookups are
not allowlisted), and anything outside the allowlist above.

Child process details: the child's stdout and stderr are pipes. They are never the caller's file descriptors.
On this machine, sandboxed Node aborted at start-up when its stdout or stderr was a regular file outside the
workspace. The child leads its own process group. When it exits, when a timeout expires, and on SIGINT, SIGTERM or
SIGHUP sent to the runner, the whole group is killed, so processes the command started do not outlive the run. Output
pipes are closed 2 seconds after the child exits, so a process that left the group cannot hang the runner.

## 5. What was verified

Command, from the project root:

    node --test 'tests/security/build-isolation/*.test.mjs'

Last run: 2026-10-08, macOS 27.0.1 (26A434), arm64, Node v26.5.1, Rust 1.99.0: 27 passed, 0 failed, 0 skipped. Three
consecutive runs gave the same result. Sandboxed cases are reported as loud skips, not passes, when `sandbox-exec` is
missing: in a copy whose sandbox path points nowhere, the same suite gave 12 passed, 15 loud skips, 0 failed. Mutation check: with the group
kill replaced by a kill of the direct child only, the timeout and SIGTERM tests fail. The
literal form `node --test tests/security/build-isolation/` does not work on Node v26.5.1 (it fails with "Cannot find
module"); use the glob form above.

Attack results, Node postinstall-style script and Rust build-script fixtures alike:

| Attempt | Unsandboxed control | Sandboxed |
|---|---|---|
| Read `$HOME/.vn-g0c-secret` (sentinel outside the workspace) | Succeeds, sentinel returned | EPERM (errno 1) |
| Write a file outside the workspace | Succeeds | EPERM |
| TCP connect to a localhost listener the test opened | Succeeds, listener receives the nonce | EPERM, listener accepts nothing |
| Read and write inside the workspace | Succeeds | Succeeds |

Escape attempts through the workspace: a symlink inside the workspace pointing at the sentinel can be created, but
reading through it returns EPERM. A hard link to the sentinel cannot be created (EPERM).

Negative controls: (a) a profile with `(allow file-read* file-write* network*)` appended is caught by the same
containment checks, so the sandboxed assertions can fail; (b) a no-op `sandbox-exec` stand-in is refused by the
self-test (exit 98) and the command does not run; (c) a profile with its escaping removed is rejected by
`sandbox-exec`.

The Rust fixture is compiled with the project `rustc` outside the sandbox. Compiling does not run the fixture.

## 6. Limits

- `sandbox-exec` is deprecated. The man page on this machine says so: "The sandbox-exec command is DEPRECATED."
  Apple recommends App Sandbox for applications. Future macOS releases may remove or change it. In that case the
  runner refuses to run (97). It does not fall back.
- Best effort. The profile is hand-written against sandbox operations that Apple does not fully document. It has not been audited, and
  it has not been tested against kernel or sandbox escapes, resource exhaustion, fork bombs, timing channels, or
  GPU and IOKit access.
- Re-verify on every macOS release. The results above are for macOS 27.0.1 on arm64 only.
- Existence is not hidden. For the sentinel path (inside a directory the sandbox denies), `access(F_OK)` and
  `fs.existsSync` still succeed. Stat, directory listing and file reads are denied. Verified for that path only.
- Symlinks may point outside the workspace. Following them is denied; creating them is allowed.
- Time zone data is unreadable inside the sandbox. `Intl` reports UTC unless the caller passes `TZ` with `--env`
  (tested).
- Programs in `/bin` and `/usr/bin`, and anything written into the workspace or scratch directory, can run inside
  the sandbox with the same restrictions. Their behaviour has not been reviewed.
- Compatibility: system services are unavailable inside the sandbox because no mach lookups are allowlisted.
  Tools that need DNS, keychain, logging, Apple Events or XPC are expected to fail. Not tested individually.
- Network families other than TCP to localhost were not tested. Network is denied by the profile.
- Containers and virtual machines were not verified here, and they are not assumed to isolate the host. Any such
  isolation needs its own work package and evidence.
- Process groups: a process the command starts that leaves its group (for example by calling `setsid`) is not
  killed. Its output is cut off after the 2-second grace period, but it keeps running. Not restricted by the profile.
  The runner does not limit process count or memory.
- Linux and Windows: not supported. The runner refuses to run there.
- Cargo: see rule 5 above. The tests do not run `cargo build` under the sandbox.

## 7. Files

- `tooling/sandbox/run-untrusted.mjs`: runner (CLI and function API).
- `tests/security/build-isolation/`: tests, fixtures (`postinstall.mjs`, `build-script.rs`, `escape-attempts.mjs`)
  and helpers.
- `release/dependency-policy/POLICY.md` and `candidates.json`: dependency rules and licence ledger.
