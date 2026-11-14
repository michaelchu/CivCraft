#!/usr/bin/env bash
set -euo pipefail

# Install the stable Rust toolchain in a fresh Codex Cloud environment.
if ! command -v cargo >/dev/null 2>&1; then
  if ! command -v curl >/dev/null 2>&1; then
    echo "error: curl is required to install Rust" >&2
    exit 1
  fi

  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain stable
fi

if [ -f "${HOME}/.cargo/env" ]; then
  # shellcheck disable=SC1091
  source "${HOME}/.cargo/env"
fi

rustup toolchain install stable --profile minimal
rustup default stable

echo "Rust toolchain ready: $(rustc --version); $(cargo --version)"
