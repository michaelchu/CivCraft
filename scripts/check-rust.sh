#!/bin/sh
set -eu

if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
  # Git clients may launch hooks without the interactive shell's Rustup PATH.
  # Load Rustup's standard Cargo environment before running the checks.
  . "$HOME/.cargo/env"
fi

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"
cargo fmt --all -- --check
cargo clippy --offline --all-targets -- -D warnings
cargo nextest run --profile ci --all-targets
cargo test --doc
