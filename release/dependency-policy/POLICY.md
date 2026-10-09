# Dependency policy

Status: draft from WP-G0C (tasks 2.3, R22 and R25). The evidence behind every "current state" line is in
`candidates.json` (snapshot 2026-10-08). The automated release check in section 8 does not exist yet. Until it
does, a release must not claim a licence is cleared when `candidates.json` marks the component `unresolved`.

## 1. Scope

Every third-party item the project builds, bundles, ships or executes: npm packages, Cargo crates (including
build-script and proc-macro crates), fonts, images and other assets, test corpora, and toolchain binaries.

## 2. Lockfiles and integrity

1. npm: `package-lock.json` is committed. Install with `npm ci --ignore-scripts`. Every lock entry must carry an
   `integrity` value. Current state: 23 of 23 entries do.
2. Cargo: every buildable crate commits its `Cargo.lock` and builds with `--locked`. Current state: the prototype
   `generated/Cargo.lock` pins 364 registry crates with checksums.
3. Toolchain: `rust-toolchain.toml` pins Rust 1.99.0. A release must record the compiler version and checksums of
   the toolchain it used. Not yet recorded.
4. A checksum or integrity mismatch is a hard failure. Do not regenerate a lock to clear a mismatch without a
   written reason.

## 3. Registry allowlist

- npm: `registry.npmjs.org` only.
- Cargo: `index.crates.io` (sparse index) and `static.crates.io` (crate downloads) only.
- Git sources, tarball URLs, `file:` specifiers, alternative registries and mirrors, and path dependencies that
  point outside this repository are refused unless the coordinator records a decision in `candidates.json`.
- A package name that is not in the lock is never installed from another source "to be safe".

## 4. Version pinning

- No floating versions in manifests for anything shipped or used in a release: no `latest`, `*`, `x`, open-ended
  `>=` ranges, `^` or `~` ranges. Direct npm dependencies are exact (`3.5.43`). Direct Cargo dependencies are exact
  (`=x.y.z`). The lock pins transitive versions.
- Upgrades are separate changes that review the lock diff.

## 5. Install and build scripts

1. npm lifecycle scripts (`preinstall`, `install`, `postinstall`, `prepare`) are disabled by default. Current
   state: no locked npm package declares an install script.
2. A package that needs a script to build (for example a native addon) runs it only inside the untrusted-build
   sandbox (`tooling/sandbox/run-untrusted.mjs`), after review. The package version and script text are recorded
   in `candidates.json`.
3. Cargo `build.rs` scripts and proc-macros run code at build time. A crate with either is untrusted until
   reviewed. Current state: 26 of the 172 cached registry crates contain a `build.rs`, and 11 are proc-macro
   crates. Cargo is not a sandbox (`docs/security/build-trust.md`).

## 6. Licences

Categories apply to each SPDX identifier in a package's metadata and its licence files. The project's own name
does not matter.

- Allowed, with notices kept: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, 0BSD, Unicode-3.0.
- Review required, with a recorded decision that names the exact file and a second reviewer: MPL-2.0 (file-level
  copyleft; design section 15 flags Stylo files), LGPL-2.1 and LGPL-3.0 (linking conditions), EPL-2.0, CC-BY-4.0
  (non-code content), OFL-1.1 and other font licences, Apache-2.0 with LLVM exception, and any `OR` expression
  whose chosen option must be written down.
- Denied by default: GPL-2.0, GPL-3.0 and their `-only` and `-or-later` forms, AGPL-3.0, SSPL, BUSL, the Commons
  Clause, non-commercial Creative Commons licences, "free for non-commercial use" and conditional royalty-free
  terms, and `UNLICENSED` for third-party code. Slint's GPL, royalty-free and commercial options and Qt's module
  licences are denied by default. Selecting a commercial option is a separate coordinator decision and is not
  part of research work.
- Unknown: a package with no SPDX identifier, no licence file, or licence text that cannot be matched fails the
  release check. Nothing is silently omitted (design section 15).
- Fonts and images need their own redistribution permission recorded, in addition to the code licence.

## 7. Adding a dependency

1. A work package must list the dependency (`docs/dev/AGENT-CONTRACT.md`). Otherwise raise a coordinator request.
2. Add or update the entry in `candidates.json` as `unresolved`. Quote the design.md signal verbatim.
3. Pin the exact version, update the lock, and record the integrity value or checksum.
4. Read the licence from the locked artifact: the LICENSE file in `node_modules/<package>/` or in the cargo
   registry cache. Record its path and sha256. Memory, web pages and `Cargo.toml` metadata alone are not evidence.
5. Mark `verified` only when every locked package in the component's graph has a local licence file. Otherwise
   keep `unresolved` and say what is missing.
6. Keep the copyright and notice lines that the licences require. A notices generator is future work.
7. Review-required categories need a second reviewer's sign-off, recorded in `candidates.json`.
8. Research work packages do not create accounts, make purchases or publish anything.

## 8. Release check (not implemented)

The release check must fail when: a shipped component is `unresolved`; a licence is unknown or denied; a lock or
integrity value does not match; a registry is outside section 3; a floating version appears (section 4); or
install scripts are enabled. Building that check is a later work package.

## 9. Current state (2026-10-08)

- Verified: Vue 3.5.43 packages and `@babel/parser` 7.29.9, with the 23 locked npm packages that make up their
  graph (`candidates.json`, `npmLockedGraph`).
- Unresolved: every Rust candidate (egui/eframe, winit, wgpu, Taffy, Parley/Vello, Blitz/Stylo, AccessKit, font,
  image and SVG libraries). Only 172 of 364 locked Rust crates are present locally, and several crate directories
  have metadata but no licence file. Also unresolved: Slint/Qt, SQLite, TLS, Test262/WPT, and the project's own
  source ownership (no LICENSE file in the repository).
