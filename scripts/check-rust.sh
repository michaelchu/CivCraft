#!/bin/sh
set -eu

repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"
cargo fmt --all -- --check
cargo clippy --offline --all-targets -- -D warnings
cargo nextest run --profile ci --all-targets
cargo test --doc
