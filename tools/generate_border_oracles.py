#!/usr/bin/env python3
"""Generate the Civ2Civ3 border vectors from a local Freeciv server.

Example:
  python3 tools/generate_border_oracles.py --server /path/freeciv-server \
    --data /path/freeciv/data --source-root /path/freeciv \
    --work /tmp/civcraft-border-oracles \
    --output tests/fixtures/freeciv_border_vectors.tsv

The native map uses the campaign's 36x48 dimensions and WrapX. The
``civ3_topology`` rows are derived from the native single-city result using
CivCraft's eight-direction topology; all other rows are native save output.
"""

import argparse
import hashlib
import os
from pathlib import Path
import re
import selectors
import subprocess
import time


REVISION = "9ff184b6eb128ca636c7ad6269e27aa959600006"
SERVER_VERSION = "3.3.90.17-dev"
SERVER_SHA256 = "63b810467344afb42de73451bf6f6760da51b1318bd4dee4ddbb1b4a0b707389"
SERVER_LIBRARY_SHA256 = "05d8cbcb520f0a5916834728a38db2b1ebc994a9ef3cd0c39ba67c3db2fa5d5a"
SETUP_LUA = r'''print("BORDER_CASES_BEGIN")
local p0 = find.player(0)
local p1 = find.player(1)
for tile in whole_map_iterate() do
  if tile then
    tile:change_terrain(find.terrain("Grassland"))
    tile:show(p0)
    tile:show(p1)
  end
end
local function create_sized_city(player, x, y, name, size)
  local tile = find.tile(x, y)
  edit.city_create(player, tile, name, nil)
  local city = tile:city()
  if size > 8 then
    edit.create_building(city, find.building_type("Aqueduct"))
  end
  if size > city.size then
    edit.change_city_size(city, size - city.size, player)
  end
  if city.size ~= size then
    error("city size setup failed for " .. name .. ": expected " .. size
      .. ", got " .. city.size)
  end
  print("CITY", name, x, y, city.size)
end
-- Create in increasing native tile-index order, matching Freeciv's scan.
create_sized_city(p0, 4, 6, "Radius", 1)
create_sized_city(p0, 18, 8, "Strong old", 1)
create_sized_city(p1, 20, 12, "Strong new", 5)
create_sized_city(p0, 18, 22, "Tie old", 1)
create_sized_city(p1, 20, 26, "Tie new", 1)
create_sized_city(p0, 24, 32, "Permanent old", 1)
create_sized_city(p1, 22, 36, "Permanent new", 12)
local permanent_tile = find.tile(23, 33)
local old_tile = find.tile(24, 32)
local challenger_tile = find.tile(22, 36)
local old_city = old_tile:city()
local challenger = challenger_tile:city()
local old_distance = old_tile:sq_distance(permanent_tile)
local challenger_distance = challenger_tile:sq_distance(permanent_tile)
-- Pinned civ2civ3 game.ruleset: radius_sq_city=5,
-- size_effect=1, radius_sq_city_permanent=-3.
local permanent_radius = 5 - 3
local challenger_radius = 5 + math.min(challenger.size, 26)
local old_strength = math.floor((old_city.size + 2) ^ 2 / old_distance)
local challenger_strength = math.floor((challenger.size + 2) ^ 2 / challenger_distance)
if old_distance > permanent_radius then
  error("permanent challenger test target is not inside the old city's protected radius")
end
if challenger_distance > challenger_radius then
  error("permanent challenger test is outside the challenger's border radius")
end
if challenger_strength <= old_strength then
  error("permanent challenger test does not have a stronger challenger")
end
print("PERMANENT_CHALLENGE", old_distance, challenger_distance,
      old_strength, challenger_strength)
server.save("border-cases.sav")
print("BORDER_CASES_DONE")
'''


def run_reference(server: Path, data: Path, work: Path) -> str:
    work.mkdir(parents=True, exist_ok=True)
    save = work / "border-cases.sav"
    if save.exists():
        save.unlink()
    setup = work / "border_setup.lua"
    setup.write_text(SETUP_LUA)
    env = {**os.environ, "FREECIV_DATA_PATH": str(data)}
    proc = subprocess.Popen(
        [str(server), "-A", "none", "-b", "127.0.0.1", "-p", "0", "-s", str(work)],
        cwd=work,
        env=env,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    assert proc.stdin is not None and proc.stdout is not None
    selector = selectors.DefaultSelector()
    selector.register(proc.stdout, selectors.EVENT_READ)
    output = bytearray()
    started = False
    script_sent = False
    finished = False
    deadline = time.monotonic() + 45
    commands = [
        "rulesetdir civ2civ3",
        "set mapsize XYSIZE",
        "set xsize 36",
        "set ysize 48",
        "set wrap WRAPX",
        "set mapseed 42",
        "set gameseed 42",
        "set aifill 2",
        "set minplayers 0",
        "set startunits c",
        "set compresstype PLAIN",
        "start",
    ]
    try:
        while time.monotonic() < deadline:
            for key, _ in selector.select(0.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    continue
                output.extend(chunk)
                if not started and b"Now accepting new client connections" in output:
                    for command in commands:
                        proc.stdin.write((command + "\n").encode())
                        proc.stdin.flush()
                        time.sleep(0.12)
                    started = True
                if started and not script_sent and b"Game saved as" in output:
                    proc.stdin.write(b"lua unsafe-file border_setup.lua\n")
                    proc.stdin.flush()
                    script_sent = True
                if script_sent and b"BORDER_CASES_DONE" in output:
                    proc.stdin.write(b"quit\n")
                    proc.stdin.flush()
                    finished = True
                    break
                if script_sent and b"lua error:" in output:
                    raise RuntimeError("native Lua fixture setup failed")
            if finished or proc.poll() is not None:
                break
        if not finished:
            raise TimeoutError(f"native border generation timed out in {work}")
        proc.wait(timeout=10)
        if proc.returncode != 0:
            raise RuntimeError(f"Freeciv exited with status {proc.returncode}")
    finally:
        selector.close()
        (work / "server.log").write_bytes(output)
        if proc.poll() is None:
            proc.terminate()
            proc.wait(timeout=5)
    if not save.exists():
        raise RuntimeError(f"native Freeciv save was not created: {save}")
    return save.read_text()


def verify_inputs(server: Path, data: Path, source_root: Path) -> None:
    source_root = source_root.resolve()
    data = data.resolve()
    if data != source_root / "data":
        raise ValueError("--data must be the data directory inside --source-root")

    revision = subprocess.run(
        ["git", "-C", str(source_root), "rev-parse", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if revision != REVISION:
        raise ValueError(f"expected Freeciv source revision {REVISION}, got {revision}")
    dirty_data = subprocess.run(
        [
            "git",
            "-C",
            str(source_root),
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            "data/civ2civ3",
        ],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if dirty_data:
        raise ValueError("pinned data/civ2civ3 ruleset has local changes")

    version = subprocess.run(
        [str(server), "--version"],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        timeout=10,
    ).stdout
    if f"Freeciv version {SERVER_VERSION}" not in version:
        raise ValueError(f"expected Freeciv server {SERVER_VERSION}, got {version.strip()!r}")
    digest = hashlib.sha256(server.read_bytes()).hexdigest()
    if digest != SERVER_SHA256:
        raise ValueError(
            f"expected pinned server SHA-256 {SERVER_SHA256}, got {digest}"
        )
    server_library = server.parent / "libfreeciv.dylib"
    if not server_library.is_file():
        raise ValueError(f"pinned Freeciv shared library is missing: {server_library}")
    library_digest = hashlib.sha256(server_library.read_bytes()).hexdigest()
    if library_digest != SERVER_LIBRARY_SHA256:
        raise ValueError(
            "expected pinned Freeciv shared-library SHA-256 "
            f"{SERVER_LIBRARY_SHA256}, got {library_digest}"
        )


def read_border_map(
    text: str,
) -> tuple[int, int, list[list[int | None]], list[list[int | None]]]:
    section = text.split("[map]\n", 1)[1].split("\n[", 1)[0]
    owner_rows = {
        int(y): [None if value == "-" else int(value) for value in row.split(",")]
        for y, row in re.findall(r'^owner(\d+)="([^"]*)"$', section, re.M)
    }
    source_rows = {
        int(y): [None if value == "-" else int(value) for value in row.split(",")]
        for y, row in re.findall(r'^source(\d+)="([^"]*)"$', section, re.M)
    }
    if not owner_rows or set(owner_rows) != set(source_rows):
        raise ValueError("save has incomplete map.owner/map.source rows")
    width = len(owner_rows[0])
    height = len(owner_rows)
    if width != 36 or height != 48 or any(
        len(row) != width for row in owner_rows.values()
    ):
        raise ValueError(f"expected a 36x48 native map, got {width}x{height}")
    return width, height, [owner_rows[y] for y in range(height)], [source_rows[y] for y in range(height)]


def civ3_distance(ax: int, ay: int, bx: int, by: int, width: int) -> int:
    start_x = 2 * ax + ay % 2
    target_x = 2 * bx + by % 2
    period = 2 * width
    return min(
        (abs(target_x + wrap * period - start_x) + abs(by - ay)) // 2
        for wrap in (-1, 0, 1)
    )


def generate_rows(text: str) -> list[str]:
    width, height, owners, sources = read_border_map(text)
    radius_source = 4 + 6 * width
    radius_native = {
        (x, y)
        for y in range(height)
        for x in range(width)
        if owners[y][x] == 0 and sources[y][x] == radius_source
    }
    if not radius_native:
        raise ValueError("native Freeciv produced no claims for the radius city")
    rows = []
    for x, y in sorted(radius_native, key=lambda point: (point[1], point[0])):
        rows.append(f"radius\t{x}\t{y}\t0\t4\t6\tnative")

    radius_civ3 = {
        (x, y)
        for y in range(height)
        for x in range(width)
        if civ3_distance(4, 6, x, y, width) ** 2 <= 6
    }
    for x, y in sorted(radius_civ3 - radius_native, key=lambda point: (point[1], point[0])):
        rows.append(f"radius\t{x}\t{y}\t0\t4\t6\tciv3_topology")

    points = {
        "strong": (19, 10, 1, 20, 12),
        "tie": (19, 24, 0, 18, 22),
        "permanent": (23, 33, 0, 24, 32),
    }
    for case, (x, y, expected_owner, expected_x, expected_y) in points.items():
        owner = owners[y][x]
        source_id = sources[y][x]
        if owner != expected_owner or source_id != expected_x + expected_y * width:
            raise ValueError(
                f"native {case} output changed at ({x},{y}): owner={owner}, source={source_id}"
            )
        rows.append(f"{case}\t{x}\t{y}\t{owner}\t{expected_x}\t{expected_y}\tnative")
    return rows


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--server", type=Path, required=True)
    parser.add_argument("--data", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    server = args.server.resolve()
    data = args.data.resolve()
    verify_inputs(server, data, args.source_root)
    text = run_reference(server, data, args.work.resolve())
    rows = generate_rows(text)
    header = [
        f"# Freeciv server {SERVER_VERSION}; binary SHA-256 {SERVER_SHA256}; library SHA-256 {SERVER_LIBRARY_SHA256}; source revision {REVISION}; ruleset civ2civ3.",
        "# Native map 36x48, ISO|HEX, WrapX, seed 42; all tiles Grassland and revealed.",
        "# Native rows come from map.owner/map.source. civ3_topology rows are the explicit CivCraft topology expansion.",
        "# case\tx\ty\towner\tsource_x\tsource_y\tbasis",
    ]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join([*header, *rows]) + "\n")
    print(f"wrote {len(rows)} border vectors to {args.output}")


if __name__ == "__main__":
    main()
