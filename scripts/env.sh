# Source this file:  source scripts/env.sh   (from the project root, in bash or zsh)
# Puts the project-local Rust toolchain on PATH. Nothing outside the project is modified.
_vn_here="${BASH_SOURCE[0]:-${(%):-%x}}"
export VN_ROOT="$(cd "$(dirname "$_vn_here")/.." && pwd)"
unset _vn_here
export RUSTUP_HOME="$VN_ROOT/toolchain/rustup"
export CARGO_HOME="$VN_ROOT/toolchain/cargo"
export PATH="$CARGO_HOME/bin:$PATH"
# Disk is tight on the dev machine: no incremental caches, no debuginfo (see Cargo.toml profiles).
export CARGO_INCREMENTAL=0
# Each work package should set its own target dir, e.g.  export CARGO_TARGET_DIR="$VN_ROOT/target/wp-l1"
: "${CARGO_TARGET_DIR:=$VN_ROOT/target/shared}"
export CARGO_TARGET_DIR
export npm_config_cache="$VN_ROOT/.npm-cache"
