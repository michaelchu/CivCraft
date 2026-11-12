#!/usr/bin/env python3
"""Export raw Conquests BLDG and TECH records to source-backed TSV catalogs.

The input is the uncompressed byte stream from Conquests/conquests.biq. Keep
BIQ record indices and raw reference indices intact; runtime crosswalks belong
in the rules adapter, not in this source extraction step.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import struct
from pathlib import Path

from export_civ3_conquests_culture import CIVCRAFT_BUILDING_INDEX_BY_BIQ_INDEX


BUILDING_FIELDS = [
    "id", "source_record_index", "source_biq", "name", "civilopedia_entry",
    "doubles_happiness", "gain_in_every_city", "gain_in_every_city_on_continent",
    "required_building_index", "cost", "culture", "bombard_defense",
    "naval_bombard_defense", "defense_bonus", "naval_defense_bonus",
    "maintenance_cost", "content_faces_all_cities", "content_faces",
    "unhappy_faces_all_cities", "unhappy_faces", "number_of_required_buildings",
    "air_power", "naval_power", "pollution", "production", "required_government",
    "spaceship_part", "required_advance_index", "rendered_obsolete_by_advance_index",
    "required_resource_1_index", "required_resource_2_index", "flags_hex",
    "number_of_armies_required", "flavors", "unknown_tail_hex", "unit_produced_index",
    "unit_frequency", "required_building_name", "required_advance_name",
    "rendered_obsolete_by_advance_name", "required_resource_1_name",
    "required_resource_2_name", "unit_produced_name",
]

TECH_FIELDS = [
    "id", "source_record_index", "source_biq", "name", "civilopedia_entry",
    "cost", "era", "advance_icon", "x", "y", "prerequisite_1_index",
    "prerequisite_2_index", "prerequisite_3_index", "prerequisite_4_index",
    "flags_hex", "flavors", "unknown_tail_hex", "prerequisite_1_name",
    "prerequisite_2_name", "prerequisite_3_name", "prerequisite_4_name",
]

BUILDING_BINDING_FIELDS = [
    "civ3_id", "source_record_index", "legacy_runtime_index", "mapping_status", "mapping_note",
]


def text_at(data: bytes, offset: int, size: int) -> str:
    return data[offset:offset + size].split(b"\0", 1)[0].decode("cp1252").strip()


def section(data: bytes, tag: bytes) -> tuple[int, int]:
    offset = data.find(tag)
    if offset < 0 or offset + 8 > len(data):
        raise ValueError(f"BIQ section {tag!r} not found or truncated")
    return offset, struct.unpack_from("<I", data, offset + 4)[0]


def fixed_names(data: bytes, tag: bytes, name_offset: int, name_size: int) -> list[str]:
    offset, count = section(data, tag)
    cursor = offset + 8
    values = []
    for index in range(count):
        length = struct.unpack_from("<i", data, cursor)[0]
        if length < name_offset + name_size - 4 or cursor + length + 4 > len(data):
            raise ValueError(f"Invalid {tag.decode()} record length at index {index}")
        values.append(text_at(data, cursor + name_offset, name_size))
        cursor += length + 4
    return values


def record_index(value: int, rows: list[str], label: str) -> str:
    if value == -1:
        return ""
    if not 0 <= value < len(rows):
        raise ValueError(f"Invalid {label} index {value}; expected -1 or < {len(rows)}")
    return rows[value]


def read_technologies(data: bytes, biq_sha256: str) -> tuple[list[dict[str, object]], list[str], int]:
    offset, count = section(data, b"TECH")
    cursor = offset + 8
    rows = []
    names: list[str] = []
    for index in range(count):
        length = struct.unpack_from("<i", data, cursor)[0]
        if length != 112 or cursor + length + 4 > len(data):
            raise ValueError(f"Unexpected TECH record length at index {index}: {length}")
        name = text_at(data, cursor + 4, 32)
        civilopedia = text_at(data, cursor + 36, 32)
        cost, era, icon, x, y = struct.unpack_from("<5i", data, cursor + 68)
        prerequisites = struct.unpack_from("<4i", data, cursor + 88)
        flags = data[cursor + 104:cursor + 108]
        flavors = struct.unpack_from("<i", data, cursor + 108)[0]
        unknown = data[cursor + 112:cursor + 116]
        if not name or not civilopedia:
            raise ValueError(f"TECH[{index}] is missing its name or Civilopedia key")
        names.append(name)
        rows.append({
            "id": civilopedia,
            "source_record_index": index,
            "source_biq": f"Conquests/conquests.biq#TECH[{index}]@{biq_sha256}",
            "name": name,
            "civilopedia_entry": civilopedia,
            "cost": cost,
            "era": era,
            "advance_icon": icon,
            "x": x,
            "y": y,
            **{f"prerequisite_{n}_index": value for n, value in enumerate(prerequisites, 1)},
            "flags_hex": flags.hex(),
            "flavors": flavors,
            "unknown_tail_hex": unknown.hex(),
            **{f"prerequisite_{n}_name": "" for n in range(1, 5)},
        })
        cursor += length + 4

    if count != 83 or len(set(names)) != count or len({row["id"] for row in rows}) != count:
        raise ValueError(f"Expected 83 unique Conquests TECH records; found {count}")
    for row in rows:
        for n in range(1, 5):
            value = int(row[f"prerequisite_{n}_index"])
            row[f"prerequisite_{n}_name"] = record_index(value, names, "TECH prerequisite")
    return rows, names, cursor


def read_buildings(
    data: bytes,
    biq_sha256: str,
    tech_names: list[str],
    resource_names: list[str],
    unit_names: list[str],
) -> tuple[list[dict[str, object]], int]:
    offset, count = section(data, b"BLDG")
    cursor = offset + 8
    rows = []
    names: list[str] = []
    for index in range(count):
        length = struct.unpack_from("<i", data, cursor)[0]
        if length != 268 or cursor + length + 4 > len(data):
            raise ValueError(f"Unexpected BLDG record length at index {index}: {length}")
        name = text_at(data, cursor + 68, 32)
        civilopedia = text_at(data, cursor + 100, 32)
        if not name or not civilopedia:
            raise ValueError(f"BLDG[{index}] is missing its name or Civilopedia key")
        names.append(name)
        scalars = struct.unpack_from("<26i", data, cursor + 132)
        flags = data[cursor + 236:cursor + 252]
        number_of_armies, flavors = struct.unpack_from("<2i", data, cursor + 252)
        unknown_tail = data[cursor + 260:cursor + 264]
        unit_produced, unit_frequency = struct.unpack_from("<2i", data, cursor + 264)
        row: dict[str, object] = {
            "id": civilopedia,
            "source_record_index": index,
            "source_biq": f"Conquests/conquests.biq#BLDG[{index}]@{biq_sha256}",
            "name": name,
            "civilopedia_entry": civilopedia,
            **dict(zip(BUILDING_FIELDS[5:31], scalars)),
            "flags_hex": flags.hex(),
            "number_of_armies_required": number_of_armies,
            "flavors": flavors,
            "unknown_tail_hex": unknown_tail.hex(),
            "unit_produced_index": unit_produced,
            "unit_frequency": unit_frequency,
        }
        row["required_building_name"] = ""
        for key, refs in [
            ("required_advance_name", tech_names),
            ("rendered_obsolete_by_advance_name", tech_names),
            ("required_resource_1_name", resource_names),
            ("required_resource_2_name", resource_names),
            ("unit_produced_name", unit_names),
        ]:
            index_key = {
                "required_advance_name": "required_advance_index",
                "rendered_obsolete_by_advance_name": "rendered_obsolete_by_advance_index",
                "required_resource_1_name": "required_resource_1_index",
                "required_resource_2_name": "required_resource_2_index",
                "unit_produced_name": "unit_produced_index",
            }[key]
            row[key] = record_index(int(row[index_key]), refs, index_key)
        rows.append(row)
        cursor += length + 4

    if count != 83 or len(set(names)) != count or len({row["id"] for row in rows}) != count:
        raise ValueError(f"Expected 83 unique Conquests BLDG records; found {count}")
    for row in rows:
        row["required_building_name"] = record_index(
            int(row["required_building_index"]), names, "BLDG prerequisite"
        )
    return rows, cursor


def write_tsv(path: Path, fields: list[str], rows: list[dict[str, object]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--unpacked-biq", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, default=Path("../source"))
    parser.add_argument("--output-dir", type=Path, default=Path("data/rulesets/civ3_conquests"))
    args = parser.parse_args()

    source = args.source_root.resolve()
    biq_path = source / "Conquests/conquests.biq"
    if not biq_path.is_file():
        raise FileNotFoundError(f"Conquests BIQ not found: {biq_path}")
    biq_sha256 = hashlib.sha256(biq_path.read_bytes()).hexdigest()
    data = args.unpacked_biq.read_bytes()
    tech_rows, tech_names, tech_end = read_technologies(data, biq_sha256)
    unit_names = fixed_names(data, b"PRTO", 8, 32)
    resource_names = fixed_names(data, b"GOOD", 4, 24)
    building_rows, building_end = read_buildings(
        data, biq_sha256, tech_names, resource_names, unit_names
    )

    # The BIQ stores each record with a four-byte length prefix. Confirm the
    # parsed section ends at its declared fixed-record boundary.
    for tag, end in [(b"TECH", tech_end), (b"BLDG", building_end)]:
        offset, _ = section(data, tag)
        if end <= offset + 8 or end > len(data):
            raise ValueError(f"Invalid {tag.decode()} section end {end}")

    write_tsv(args.output_dir / "technologies.tsv", TECH_FIELDS, tech_rows)
    write_tsv(args.output_dir / "buildings.tsv", BUILDING_FIELDS, building_rows)
    binding_rows = []
    for row in building_rows:
        source_index = int(row["source_record_index"])
        runtime_index = CIVCRAFT_BUILDING_INDEX_BY_BIQ_INDEX.get(source_index)
        if runtime_index is not None and not 0 <= runtime_index <= 90:
            raise ValueError(f"Invalid stable Building wrapper index {runtime_index} for BLDG[{source_index}]")
        binding_rows.append({
            "civ3_id": row["id"],
            "source_record_index": source_index,
            "legacy_runtime_index": "" if runtime_index is None else runtime_index,
            "mapping_status": "mapped" if runtime_index is not None else "unmapped",
            "mapping_note": "" if runtime_index is not None else "No legacy Building wrapper has been assigned to this BIQ record.",
        })
    write_tsv(args.output_dir / "building_catalog_bindings.tsv", BUILDING_BINDING_FIELDS, binding_rows)
    print(f"Wrote {len(building_rows)} BLDG and {len(tech_rows)} TECH records")
    print(f"Legacy Building wrapper crosswalk: {sum(row['mapping_status'] == 'mapped' for row in binding_rows)}/{len(binding_rows)} mapped")
    print(f"Source: {biq_path} sha256={biq_sha256}")


if __name__ == "__main__":
    main()
