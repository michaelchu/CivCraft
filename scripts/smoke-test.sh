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

smoke_args=(--smoke-test --smoke-turns "$turns" --seed 42 --map-size tiny)
if [[ "$(uname -s)" == "Darwin" ]]; then
    # LaunchServices gives Macroquad the same desktop-session path as a normal
    # Finder-launched app. Direct shell launches can lack the active AppKit /
    # Metal session and fail before the first frame is rendered.
    bundle_root=$(mktemp -d "$app_dir/target/civcraft-smoke.XXXXXX")
    bundle="$bundle_root/CivCraft.app"
    stdout_log="$bundle_root/stdout.log"
    stderr_log="$bundle_root/stderr.log"
    trap 'rm -rf "$bundle_root"' EXIT

    mkdir -p "$bundle/Contents/MacOS"
    cargo build --offline --release --features desktop-audio --bin civcraft
    cp target/release/civcraft "$bundle/Contents/MacOS/civcraft"
    cp scripts/macos-smoke/Info.plist "$bundle/Contents/Info.plist"
    codesign --force --deep --sign - "$bundle" >/dev/null

    open -n -W \
        --stdout "$stdout_log" \
        --stderr "$stderr_log" \
        --env "CIVCRAFT_SOURCE_DIR=$source_dir" \
        "$bundle" --args "${smoke_args[@]}"
    cat "$stdout_log"
    cat "$stderr_log" >&2
    if ! rg -q "^Smoke test passed:" "$stdout_log"; then
        echo "Desktop smoke test did not report success." >&2
        exit 1
    fi
else
    cargo run --offline --release --features desktop-audio --bin civcraft -- \
        "${smoke_args[@]}"
fi
