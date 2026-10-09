#!/bin/bash
# Private working copy for agents that share a Cargo crate with others (so a half-written file of one agent never breaks another's build).
#   scripts/mirror.sh init <wp> <owned-path>...   create target/mirrors/<wp> (a copy of the repo subset) and record your owned paths
#   scripts/mirror.sh pull <wp>                   refresh the mirror from the repo, EXCEPT your owned paths (picks up others' finished work)
#   scripts/mirror.sh push <wp>                   copy your owned paths from the mirror back into the repo (do this only when they compile and pass)
#   scripts/mirror.sh path <wp>                   print the mirror directory
# Work inside the mirror (cd "$(scripts/mirror.sh path <wp>)"; source scripts/env.sh; cargo test -p <crate>). Paths are relative to the repo root.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cmd="${1:?usage}"; wp="${2:?wp id}"; shift 2 || true
M="$ROOT/target/mirrors/$wp"
SUBSET=(Cargo.toml Cargo.lock rust-toolchain.toml crates tests/language tests/dom tests/css tests/ui scripts docs/dev tooling compiler)
sync_in() {  # repo -> mirror, excluding owned paths and build output
  mkdir -p "$M"
  for s in "${SUBSET[@]}"; do
    [ -e "$ROOT/$s" ] || continue
    local parent; parent="$(dirname "$s")"
    local ex=(--exclude target --exclude node_modules --exclude '.DS_Store')
    if [ -f "$M/.owned" ]; then
      while read -r p; do
        [ -z "$p" ] && continue
        # rsync patterns are relative to the transfer root, which is the PARENT of the copied entry
        if [ "$parent" = "." ]; then rel="$p"; else rel="${p#"$parent"/}"; fi
        case "$p" in "$s"|"$s"/*) ex+=(--exclude "/$rel") ;; esac
      done < "$M/.owned"
    fi
    mkdir -p "$M/$parent"
    rsync -rlpgoDc --delete "${ex[@]}" "$ROOT/$s" "$M/$parent/"
  done
  ln -sfn "$ROOT/node_modules" "$M/node_modules"
  ln -sfn "$ROOT/toolchain" "$M/toolchain"
}
case "$cmd" in
  init)
    mkdir -p "$M"; : > "$M/.owned"
    for p in "$@"; do echo "${p%/}" >> "$M/.owned"; done
    sync_in
    # owned paths start from the repo's current content (stubs/existing files) so the first build works
    while read -r p; do [ -n "$p" ] && [ -e "$ROOT/$p" ] && mkdir -p "$M/$(dirname "$p")" && rsync -rlpgoDc "$ROOT/$p" "$M/$(dirname "$p")/"; done < "$M/.owned"
    echo "$M" ;;
  pull) [ -d "$M" ] || { echo "no mirror; run init" >&2; exit 1; }; sync_in; echo "pulled (owned paths untouched)" ;;
  push) [ -f "$M/.owned" ] || { echo "no mirror; run init" >&2; exit 1; }
    while read -r p; do [ -z "$p" ] && continue; [ -e "$M/$p" ] || continue; mkdir -p "$ROOT/$(dirname "$p")"; rsync -rlpgoDc "$M/$p" "$ROOT/$(dirname "$p")/"; echo "pushed $p"; done < "$M/.owned" ;;
  path) echo "$M" ;;
  *) echo "unknown command" >&2; exit 2 ;;
esac
