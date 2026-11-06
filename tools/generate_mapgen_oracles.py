#!/usr/bin/env python3
"""Generate map goldens with an unmodified local Freeciv server (no network peers).

Example: python3 tools/generate_mapgen_oracles.py --server /path/freeciv-server
  --data /path/freeciv/data --work /tmp/civcraft-mapgen-oracles
  --output tests/fixtures/freeciv_mapgen_vectors.tsv
"""
import argparse
import os
from pathlib import Path
import re
import selectors
import subprocess
import time


def read_map(path):
    text = path.read_text()
    section = text.split('[map]\n', 1)[1].split('\n[', 1)[0]
    terrains = dict((int(y), row) for y, row in re.findall(r'^t(\d+)="(.*)"$', section, re.M))
    height = len(terrains)
    width = len(terrains[0])
    altitudes = dict((int(y), list(map(int, row.split(',')))) for y, row in re.findall(r'^alt(\d+)="(.*)"$', section, re.M))
    extras = {(int(group), int(y)): row for group, y, row in re.findall(r'^e(\d+)_(\d+)="(.*)"$', section, re.M)}
    groups = max(group for group, _ in extras) + 1
    checksum = 0xcbf29ce484222325
    for y in range(height):
        assert len(terrains[y]) == width and len(altitudes[y]) == width
        for x in range(width):
            mask = sum(int(extras[group, y][x], 16) << (4 * group) for group in range(groups))
            for value in (ord(terrains[y][x]), altitudes[y][x], mask):
                checksum = ((checksum ^ value) * 0x100000001b3) & ((1 << 64) - 1)
    start_table = section.split('startpos={', 1)[1].split('\n}', 1)[0]
    starts = sorted((int(x), int(y)) for x, y in re.findall(r'^(\d+),(\d+),', start_table, re.M))
    assert len(starts) == 6
    return width, height, checksum, starts


def generate(args, mode, seed):
    work = args.work / f'{mode}-{seed}'
    work.mkdir(parents=True, exist_ok=True)
    target = work / 'freeciv-T0001-Y-4000-auto.sav'
    if target.exists():
        target.unlink()
    env = {**os.environ, 'FREECIV_DATA_PATH': str(args.data)}
    proc = subprocess.Popen([str(args.server), '-A', 'none', '-b', '127.0.0.1', '-p', '0', '-s', str(work)],
                            cwd=work, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    selector = selectors.DefaultSelector()
    selector.register(proc.stdout, selectors.EVENT_READ)
    output = b''
    started = False
    finished = False
    deadline = time.monotonic() + 30
    try:
        while time.monotonic() < deadline:
            for key, _ in selector.select(.2):
                chunk = os.read(key.fileobj.fileno(), 65536)
                if not chunk:
                    break
                output += chunk
                if not started and b'Now accepting new client connections' in output:
                    wrap = 'WRAPX' if mode == 'x' else 'WRAPX|WRAPY'
                    commands = ['rulesetdir civ2civ3', f'set wrap {wrap}', f'set mapseed {seed}',
                                f'set gameseed {seed}', 'set minplayers 0', 'set compresstype PLAIN', 'start']
                    # The server console consumes one line per socket/select cycle.
                    for command in commands:
                        proc.stdin.write((command + '\n').encode())
                        proc.stdin.flush()
                        time.sleep(.1)
                    started = True
                if started and b'Game saved as' in output:
                    proc.stdin.write(b'quit\n')
                    proc.stdin.flush()
                    finished = True
                    break
            if finished or proc.poll() is not None:
                break
        if not finished:
            raise RuntimeError(f'native generation did not finish: {work / "log"}')
        proc.wait(timeout=5)
        assert proc.returncode == 0
    finally:
        if proc.poll() is None:
            proc.terminate()
            proc.wait(timeout=5)
        selector.close()
        (work / 'log').write_bytes(output)
    text = target.read_text()
    assert 'server_state="S_S_RUNNING"' in text and 'turn=1\n' in text
    return read_map(target)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--server', type=Path, required=True)
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.server = args.server.resolve()
    args.data = args.data.resolve()
    args.work = args.work.resolve()
    lines = ['# Native Freeciv 3.3.90.14-dev (eb8c703), Civ2Civ3, six players, 100 land tiles/player.',
             '# wrap seed width height fnv64(terrain,altitude,extras) sorted_start_positions']
    for mode in ['x', 'xy']:
        for seed in [1, 42, 12345, 700001, 2147483647]:
            width, height, checksum, starts = generate(args, mode, seed)
            positions = ';'.join(f'{x},{y}' for x, y in starts)
            lines.append(f'{mode}\t{seed}\t{width}\t{height}\t{checksum:016x}\t{positions}')
            print(lines[-1], flush=True)
    args.output.write_text('\n'.join(lines) + '\n')


if __name__ == '__main__':
    main()
