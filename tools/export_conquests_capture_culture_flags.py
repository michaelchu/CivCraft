#!/usr/bin/env python3
"""Export GAME.RetainCulture from every installed Conquests BIQ."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import struct
from pathlib import Path


def int_at(data: bytes, offset: int) -> int:
    return struct.unpack_from("<i", data, offset)[0]


def game_retention_values(data: bytes) -> list[tuple[int, int]]:
    section_offset = data.find(b"GAME")
    if section_offset < 0 or section_offset + 8 > len(data):
        raise ValueError("BIQ GAME section not found or truncated")

    record_count = int_at(data, section_offset + 4)
    cursor = section_offset + 8
    values = []
    for record_index in range(record_count):
        length = int_at(data, cursor)
        playable_civs = int_at(data, cursor + 12)
        if length < 16 or playable_civs < 0:
            raise ValueError(f"Invalid GAME record {record_index}")

        # GAME has a 16-byte fixed prefix, a variable playable-civ list,
        # a 5304-byte fixed block, a variable alliance list, then the
        # victory/scenario-properties block. RetainCulture is byte 1996
        # within that final block.
        final_block = cursor + 16 + playable_civs * 4 + 5304 + playable_civs * 4
        field_offset = final_block + 1996
        record_end = cursor + length + 4
        if field_offset >= record_end or record_end > len(data):
            raise ValueError(f"GAME.RetainCulture outside record {record_index}")
        raw_value = data[field_offset]
        if raw_value not in (0, 1):
            raise ValueError(
                f"Unexpected GAME.RetainCulture value {raw_value} "
                f"in record {record_index}"
            )
        values.append((record_index, raw_value))
        cursor = record_end
    return values


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--blast", type=Path, required=True, help="compiled Blast BIQ decompressor")
    parser.add_argument("--source-root", type=Path, default=Path("../source"))
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("data/rulesets/civ3_conquests/culture_capture_settings.tsv"),
    )
    args = parser.parse_args()

    source_dir = args.source_root.resolve() / "Conquests"
    biqs = sorted(source_dir.rglob("*.biq"))
    if not biqs:
        raise ValueError(f"No BIQ files found under {source_dir}")

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", encoding="utf-8", newline="") as table:
        table.write(
            "biq_path\tsource_section\tsource_record_index\t"
            "retain_culture_on_capture_raw\tretain_culture_on_capture\tbiq_sha256\n"
        )
        for biq in biqs:
            raw = biq.read_bytes()
            unpacked = subprocess.run(
                [str(args.blast.resolve())], input=raw, capture_output=True, check=True
            ).stdout
            digest = hashlib.sha256(raw).hexdigest()
            relative = biq.relative_to(source_dir).as_posix()
            for record_index, value in game_retention_values(unpacked):
                table.write(
                    f"{relative}\tGAME\t{record_index}\t{value}\t"
                    f"{str(bool(value)).lower()}\t{digest}\n"
                )
    print(f"Exported GAME.RetainCulture values for {len(biqs)} BIQ files to {args.output}")


if __name__ == "__main__":
    main()
