#!/usr/bin/env python3
"""Export culture-related records from the installed Conquests campaign BIQ.

Input is the unpacked BIQ byte stream, produced with the adjacent Blast
reference utility. The output preserves source section indices and the hash of
the installed BIQ so culture rules are auditable and reproducible.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import struct
from pathlib import Path


# Explicit adapter from the installed Conquests BLDG records to the current
# CivCraft Building catalog. These are implementation IDs, not BIQ identities.
# Missing Conquests entries stay unmapped until their catalog support is added.
CIVCRAFT_BUILDING_INDEX_BY_BIQ_INDEX = {
    0: 23, 3: 40, 5: 17, 10: 8, 11: 41, 12: 73, 20: 29,
    30: 65, 31: 51, 32: 44, 33: 57, 34: 49, 35: 76, 36: 50,
    37: 69, 38: 88, 39: 58, 40: 45, 41: 67, 42: 56, 43: 54, 44: 53,
    45: 43, 46: 78, 47: 52, 48: 77, 49: 70, 50: 59, 51: 46,
    52: 75, 53: 79, 54: 81, 55: 82, 56: 80, 57: 83, 58: 84,
    59: 85, 60: 42, 61: 31, 62: 86, 63: 87, 74: 66, 78: 64,
    79: 63, 80: 62, 81: 89,
}


def section(data: bytes, tag: bytes) -> tuple[int, int]:
    offset = data.find(tag)
    if offset < 0:
        raise ValueError(f"BIQ section {tag!r} not found")
    if offset + 8 > len(data):
        raise ValueError(f"Truncated BIQ section header {tag!r}")
    count = struct.unpack_from("<I", data, offset + 4)[0]
    return offset, count


def int_at(data: bytes, offset: int) -> int:
    return struct.unpack_from("<i", data, offset)[0]


def text_at(data: bytes, offset: int, size: int) -> str:
    return data[offset:offset + size].split(b"\0", 1)[0].decode("cp1252").strip()


def bit(value: int, index: int) -> bool:
    return bool(value & (1 << index))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--unpacked-biq", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, default=Path("../source"))
    parser.add_argument("--output", type=Path, default=Path("data/rulesets/civ3_conquests/culture_biq.json"))
    parser.add_argument(
        "--culture-table",
        type=Path,
        default=Path("data/rulesets/civ3_conquests/culture_buildings.tsv"),
    )
    parser.add_argument(
        "--victory-table",
        type=Path,
        default=Path("data/rulesets/civ3_conquests/culture_victory.tsv"),
    )
    args = parser.parse_args()

    source = args.source_root.resolve()
    biq_path = source / "Conquests/conquests.biq"
    biq_hash = hashlib.sha256(biq_path.read_bytes()).hexdigest()
    data = args.unpacked_biq.read_bytes()
    result: dict[str, object] = {
        "source_biq": "source/Conquests/conquests.biq",
        "biq_sha256": biq_hash,
    }

    # TECHNOLOGY indices in BLDG records resolve to names in the preceding
    # variable-length TECH section.
    offset, count = section(data, b"TECH")
    tech_names = []
    cursor = offset + 8
    for index in range(count):
        length = int_at(data, cursor)
        if length < 68 or cursor + length + 4 > len(data):
            raise ValueError(f"Invalid TECH record length at index {index}")
        tech_names.append(text_at(data, cursor + 4, 32))
        cursor += length + 4

    def tech_name(index: int) -> str | None:
        if index == -1:
            return None
        if not 0 <= index < len(tech_names):
            raise ValueError(f"Invalid BIQ technology index {index}")
        return tech_names[index]

    # BLDG is a fixed 272-byte record. Culture is the signed integer at +152.
    offset, count = section(data, b"BLDG")
    rows = []
    cursor = offset + 8
    for index in range(count):
        name = text_at(data, cursor + 68, 32)
        if int_at(data, cursor) != 268:
            raise ValueError(f"Unexpected BLDG record length at index {index}")
        rows.append({
            "source_section": "BLDG",
            "source_record_index": index,
            "biq_sha256": biq_hash,
            "name": name,
            "civilopedia_entry": text_at(data, cursor + 100, 32),
            "production_cost_raw": int_at(data, cursor + 148),
            "culture_per_turn": int_at(data, cursor + 152),
            "maintenance_cost": int_at(data, cursor + 172),
            "required_building_index": int_at(data, cursor + 144),
            "required_advance_index": int_at(data, cursor + 220),
            "required_advance_name": tech_name(int_at(data, cursor + 220)),
            "rendered_obsolete_by_advance_index": int_at(data, cursor + 224),
            "rendered_obsolete_by_advance_name": tech_name(int_at(data, cursor + 224)),
            "civcraft_catalog_index": CIVCRAFT_BUILDING_INDEX_BY_BIQ_INDEX.get(index),
            "catalog_mapping_status": (
                "mapped" if index in CIVCRAFT_BUILDING_INDEX_BY_BIQ_INDEX
                else "unresolved" if int_at(data, cursor + 152) > 0
                else "zero-output; catalog mapping not needed"
            ),
            "flags_hex": data[cursor + 236:cursor + 252].hex(),
        })
        cursor += 272
    result["buildings"] = rows

    # CULT defines six named culture-ratio bands and their resistance settings.
    offset, count = section(data, b"CULT")
    rows = []
    for index in range(count):
        cursor = offset + 8 + index * 92
        if int_at(data, cursor) != 88:
            raise ValueError(f"Unexpected CULT record length at index {index}")
        values = struct.unpack_from("<6i", data, cursor + 68)
        rows.append({
            "source_section": "CULT",
            "source_record_index": index,
            "biq_sha256": biq_hash,
            "name": text_at(data, cursor + 4, 64),
            "propaganda_success_chance": values[0],
            "culture_ratio_percentage": values[1],
            "culture_ratio_denominator": values[2],
            "culture_ratio_numerator": values[3],
            "initial_resistance_chance": values[4],
            "continued_resistance_chance": values[5],
        })
    result["culture_ratio_bands"] = rows

    # RULE has variable spaceship and culture-level name arrays.
    offset, count = section(data, b"RULE")
    if count != 1:
        raise ValueError(f"Expected one RULE record, found {count}")
    cursor = offset + 8
    spaceship_parts = int_at(data, cursor + 100)
    segment = cursor + 104 + spaceship_parts * 4
    culture_levels = int_at(data, segment + 160)
    level_names = [
        text_at(data, segment + 164 + index * 64, 64)
        for index in range(culture_levels)
    ]
    tail = segment + 164 + culture_levels * 64
    border_multiplier, border_factor = struct.unpack_from("<2i", data, tail)
    result["rules"] = {
        "source_section": "RULE",
        "source_record_index": 0,
        "biq_sha256": biq_hash,
        "city_size_level_names": [
            text_at(data, cursor + 4, 32),
            text_at(data, cursor + 36, 32),
            text_at(data, cursor + 68, 32),
        ],
        "maximum_level_1_city_size": int_at(data, segment + 144),
        "maximum_level_2_city_size": int_at(data, segment + 148),
        "culture_level_names": level_names,
        "border_expansion_multiplier": border_multiplier,
        "border_factor": border_factor,
    }

    # GAME's cultural-victory values follow variable playable-civ and alliance
    # arrays. Its flag bytes and fixed tail are retained as source data.
    offset, count = section(data, b"GAME")
    if count != 1:
        raise ValueError(f"Expected one GAME record, found {count}")
    cursor = offset + 8
    playable_civs = int_at(data, cursor + 12)
    flags_offset = cursor + 16 + playable_civs * 4
    flags = data[flags_offset:flags_offset + 4]
    victory_offset = flags_offset + 5304 + playable_civs * 4
    # GAME's final fixed record contains MapVisible followed by the
    # RetainCulture scenario property. The editor's GAME layout places that
    # byte 1996 bytes after the victory-limit block begins.
    retain_culture_raw = data[victory_offset + 1996]
    if retain_culture_raw not in (0, 1):
        raise ValueError(
            f"Unexpected GAME.RetainCulture value: {retain_culture_raw}"
        )
    result["game"] = {
        "source_section": "GAME",
        "source_record_index": 0,
        "biq_sha256": biq_hash,
        "default_game_rules": int_at(data, cursor + 4),
        "default_victory_conditions": int_at(data, cursor + 8),
        "cultural_victory_flag": bit(flags[0], 4),
        "culturally_linked_start": bit(flags[0], 6),
        "cultural_conversions_flag": bit(flags[1], 7),
        "retain_culture_on_capture_raw": retain_culture_raw,
        "retain_culture_on_capture": bool(retain_culture_raw),
        "one_city_culture_win": int_at(data, victory_offset + 8),
        "all_cities_culture_win": int_at(data, victory_offset + 12),
        "domination_terrain": int_at(data, victory_offset + 16),
        "domination_population": int_at(data, victory_offset + 20),
    }

    civilopedia_path = source / "Conquests/Text/Civilopedia.txt"
    civilopedia = civilopedia_path.read_text(encoding="cp1252")
    victory_text = civilopedia.split("#GCON_Victory", 1)[1]
    victory_section, description = victory_text.split("#DESC_GCON_Victory", 1)
    one_city_match = re.search(
        r"single city with\s+([\d,]+)\s+culture points", victory_section
    )
    if not one_city_match:
        raise ValueError("Conquests Civilopedia cultural-victory city threshold not found")
    map_thresholds = {
        name: int(value.replace(",", ""))
        for name, value in re.findall(
            r"^\^?(Tiny|Small|Standard|Large|Huge):\s+([\d,]+)$",
            description,
            flags=re.MULTILINE,
        )
    }
    if list(map_thresholds) != ["Tiny", "Small", "Standard", "Large", "Huge"]:
        raise ValueError(f"Unexpected Conquests map culture thresholds: {map_thresholds}")
    one_city_threshold = int(one_city_match.group(1).replace(",", ""))
    if one_city_threshold != result["game"]["one_city_culture_win"]:
        raise ValueError("Civilopedia and BIQ one-city culture thresholds disagree")
    result["cultural_victory_thresholds"] = {
        "source_section": "Conquests/Text/Civilopedia.txt#GCON_Victory",
        "biq_sha256": biq_hash,
        "one_city_culture": one_city_threshold,
        "civilization_culture_by_map_size": map_thresholds,
        "biq_standard_civilization_culture": result["game"]["all_cities_culture_win"],
    }

    # RACE is dynamic. CultureGroup is in the final fixed block after city
    # names, great-leader names, the 208-byte text block, and era filenames.
    offset, count = section(data, b"RACE")
    _, era_count = section(data, b"ERAS")
    cursor = offset + 8
    rows = []
    for index in range(count):
        length = int_at(data, cursor)
        city_count = int_at(data, cursor + 4)
        leader_count_offset = cursor + 8 + city_count * 24
        great_leader_count = int_at(data, leader_count_offset)
        text_block = leader_count_offset + 4 + great_leader_count * 32
        name = text_at(data, text_block + 128, 40)
        culture_group_offset = text_block + 208 + era_count * 520
        rows.append({
            "source_section": "RACE",
            "source_record_index": index,
            "biq_sha256": biq_hash,
            "name": name,
            "culture_group_index": int_at(data, culture_group_offset),
        })
        if cursor + length + 4 <= cursor:
            raise ValueError(f"Invalid RACE record length at index {index}")
        cursor += length + 4
    result["civilizations"] = rows

    # GOVT embeds a matrix of government-vs-government resistance modifiers.
    offset, count = section(data, b"GOVT")
    cursor = offset + 8
    government_rows = []
    resistance_rows = []
    for index in range(count):
        length = int_at(data, cursor)
        name = text_at(data, cursor + 28, 64)
        matrix_count = int_at(data, cursor + 396)
        matrix_offset = cursor + 400
        tail = matrix_offset + matrix_count * 12
        government_rows.append({
            "source_section": "GOVT",
            "source_record_index": index,
            "biq_sha256": biq_hash,
            "name": name,
            "assimilation_chance": int_at(data, tail + 4),
        })
        for other_index in range(matrix_count):
            can_bribe_raw, bribery_modifier, resistance_modifier = struct.unpack_from(
                "<3i", data, matrix_offset + other_index * 12
            )
            resistance_rows.append({
                "source_section": "GOVT",
                "source_record_index": index,
                "other_government_index": other_index,
                "biq_sha256": biq_hash,
                "can_bribe_raw_hex": f"{can_bribe_raw & 0xffffffff:08x}",
                "bribery_modifier": bribery_modifier,
                "resistance_modifier": resistance_modifier,
                "interpretation_status": "schema-derived; verify before gameplay use",
            })
        if cursor + length + 4 <= cursor:
            raise ValueError(f"Invalid GOVT record length at index {index}")
        cursor += length + 4
    result["governments"] = government_rows
    result["government_interactions"] = resistance_rows

    # Difficulty records include support, corruption, cost, and military-law
    # parameters. Export them so any proposed culture modifier can be checked
    # against the BIQ rather than inferred from a nearby field.
    offset, count = section(data, b"DIFF")
    cursor = offset + 8
    difficulty_fields = [
        "citizens_born_content",
        "maximum_government_transition_time",
        "ai_defensive_starting_units",
        "ai_offensive_starting_units",
        "extra_start_unit_1",
        "extra_start_unit_2",
        "additional_free_support",
        "unit_support_bonus_per_settlement",
        "attack_bonus_against_barbarians",
        "cost_factor",
        "percentage_of_optimal_cities",
        "ai_to_ai_trade_rate",
        "corruption_percentage",
        "military_law",
    ]
    difficulty_rows = []
    for index in range(count):
        length = int_at(data, cursor)
        if length != 120:
            raise ValueError(f"Unexpected DIFF record length at index {index}: {length}")
        values = struct.unpack_from("<14i", data, cursor + 68)
        row = {
            "source_section": "DIFF",
            "source_record_index": index,
            "biq_sha256": biq_hash,
            "name": text_at(data, cursor + 4, 64),
        }
        row.update(dict(zip(difficulty_fields, values, strict=True)))
        difficulty_rows.append(row)
        cursor += length + 4
    result["difficulty"] = difficulty_rows

    result["field_notes"] = {
        "game_cultural_victory_flag": (
            "Raw GAME flag. The same record enables default victory conditions, "
            "so this flag alone does not establish the effective standard-game setting."
        ),
        "game_retain_culture_on_capture": (
            "Raw GAME.RetainCulture scenario property. The installed Conquests "
            "readme says MPTournament enables captured-city culture retention; "
            "the detailed per-civilization accounting effect is not established."
        ),
        "government_interactions": (
            "Raw GOVT pair fields follow the adjacent parser schema. Their use "
            "for occupation resistance is not established."
        ),
        "race_culture_group_index": (
            "Raw RACE index; keep separate from accumulated culture-point rules."
        ),
        "difficulty": (
            "Raw DIFF settings are exported for checking potential difficulty effects. "
            "No field is treated as a culture modifier without supporting evidence."
        ),
    }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    with args.culture_table.open("w") as table:
        table.write(
            "biq_section\tsource_record_index\tbiq_name\tculture_per_turn\t"
            "civcraft_catalog_index\tmapping_status\tbiq_sha256\n"
        )
        for row in result["buildings"]:
            catalog_index = row["civcraft_catalog_index"]
            table.write(
                f"BLDG\t{row['source_record_index']}\t{row['name']}\t"
                f"{row['culture_per_turn']}\t"
                f"{catalog_index if catalog_index is not None else '-'}\t"
                f"{row['catalog_mapping_status']}\t{biq_hash}\n"
            )
    with args.victory_table.open("w") as table:
        table.write(
            "map_size\tone_city_culture\tcivilization_culture\t"
            "civilopedia_source\tbiq_sha256\n"
        )
        for name, threshold in map_thresholds.items():
            table.write(
                f"{name}\t{one_city_threshold}\t{threshold}\t"
                f"Conquests/Text/Civilopedia.txt#GCON_Victory\t{biq_hash}\n"
            )
    print(f"Exported Conquests culture inputs to {args.output}")


if __name__ == "__main__":
    main()
