#!/usr/bin/env bash
set -euo pipefail

app_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
source_dir=${CIVCRAFT_SOURCE_DIR:-}

if [[ -z "$source_dir" ]]; then
    for candidate in "$app_dir/../source" "$app_dir/source"; do
        if [[ -d "$candidate/Conquests/Art/Terrain" ]]; then
            source_dir=$candidate
            break
        fi
    done
fi

if [[ -z "$source_dir" || ! -d "$source_dir/Conquests/Art/Terrain" ]]; then
    echo "SKIP: Civ III source path not detected; set CIVCRAFT_SOURCE_DIR to run the desktop smoke test."
    exit 0
fi

turns=${CIVCRAFT_SMOKE_TURNS:-3}
if ! [[ "$turns" =~ ^[1-9][0-9]*$ ]]; then
    echo "CIVCRAFT_SMOKE_TURNS must be a positive integer" >&2
    exit 2
fi

export CIVCRAFT_SOURCE_DIR="$source_dir"
cd "$app_dir"
cargo run --offline --release --features desktop-audio --bin civcraft -- \
    --smoke-test --smoke-turns "$turns" --seed 42 --map-size tiny
