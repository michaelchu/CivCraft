#!/usr/bin/env python3
"""Export the stock Conquests BIQ campaign units to an editable TSV catalog.

The BIQ is PKWare DCL-compressed. Pass its uncompressed bytes (for example,
produced with the adjacent OpenCiv3 Blast reference utility) via --unpacked-biq.
The exporter preserves raw PRTO flags and prototype indices so the data can be
audited against the installed rules file instead of being hand-transcribed.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import re
import struct
from pathlib import Path


CAMPAIGN_CIVS = [
    "Barbarians", "Romans", "Egyptians", "Greeks", "Babylonians", "Germans",
    "Russians", "Chinese", "Americans", "Japanese", "French", "Indians",
    "Persians", "Aztecs", "Zulus", "Iroquois", "English", "Mongols",
    "Spanish", "Vikings", "Ottomans", "Celts", "Arabs", "Carthaginians",
    "Koreans", "Sumerians", "Hittites", "Dutch", "Portuguese", "Byzantines",
    "Inca", "Mayans",
]

UNIQUE_UNITS = {
    "Bowman": ("Babylonians", "Archer", "base"),
    "Cossack": ("Russians", "Cavalry", "base"),
    "F-15": ("Americans", "Jet Fighter", "base"),
    "Hoplite": ("Greeks", "Spearman", "base"),
    "Impi": ("Zulus", "Spearman", "base"),
    "Immortals": ("Persians", "Swordsman", "base"),
    "Jaguar Warrior": ("Aztecs", "Warrior", "base"),
    "Legionary": ("Romans", "Swordsman", "base"),
    "Man-O-War": ("English", "Frigate", "base"),
    "Mounted Warrior": ("Iroquois", "Horseman", "base"),
    "Musketeer": ("French", "Musketman", "base"),
    "Panzer": ("Germans", "Tank", "base"),
    "Rider": ("Chinese", "Knight", "base"),
    "Samurai": ("Japanese", "Knight", "base"),
    "War Chariot": ("Egyptians", "Chariot", "base"),
    "War Elephant": ("Indians", "Knight", "base"),
    "Ansar Warrior": ("Arabs", "Knight", "ptw"),
    "Berserk": ("Vikings", "Longbowman", "ptw"),
    "Conquistador": ("Spanish", "Explorer", "ptw"),
    "Gallic Swordsman": ("Celts", "Swordsman", "ptw"),
    "Hwach'a": ("Koreans", "Cannon", "ptw"),
    "Keshik": ("Mongols", "Knight", "ptw"),
    "Numidian Mercenary": ("Carthaginians", "Spearman", "ptw"),
    "Sipahi": ("Ottomans", "Cavalry", "ptw"),
    "Carrack": ("Portuguese", "Caravel", "conquests"),
    "Chasqui Scout": ("Inca", "Scout", "conquests"),
    "Dromon": ("Byzantines", "Galley", "conquests"),
    "Enkidu Warrior": ("Sumerians", "Warrior", "conquests"),
    "Javelin Thrower": ("Mayans", "Archer", "conquests"),
    "Swiss Mercenary": ("Dutch", "Pikeman", "conquests"),
    "Three-Man Chariot": ("Hittites", "Chariot", "conquests"),
}

WONDER_UNITS = {"Ancient Cavalry", "Crusader"}
ART_ALIASES = {
    "Modern Paratrooper": "Paratrooper",
    "Chasqui Scout": "Chasquis Scout",
    "Numidian Mercenary": "Libyan Mercenary",
    "Hwach'a": "Hwacha",
    "Three-Man Chariot": "Three Man Chariot",
    "AEGIS Cruiser": "Aegis Cruiser",
    "ICBM": "ICBM",
    "Army": "Army Ancient Times",
    "Leader": "Leader Ancient Times",
}
ERA_ART = {
    "Army": ["Army Ancient Times", "Army Middle Ages", "Army Industrial Ages", "Army Modern Times"],
    "Leader": ["Leader Ancient Times", "Leader Middle Ages", "Leader Industrial Ages", "Leader Modern Times"],
}

FIELDS = [
    "id", "name", "category", "expansion", "source_prototypes", "source_biq",
    "cost_shields", "population_cost", "attack", "defense", "moves",
    "bombard_strength", "bombard_range", "rate_of_fire", "operational_range",
    "capacity", "required_technology", "required_resource_1", "required_resource_2",
    "required_resource_3", "upgrade_to", "available_to", "unique_civilization",
    "replaces", "unit_type", "flags_1_hex", "flags_2_hex", "flags_3_hex",
    "hit_point_bonus", "ai_other_strategy", "source_ai_profiles", "art_folder", "art_root",
    "ignore_movement_cost_hex", "zone_of_control", "bombard_effects",
    "require_support", "enslave_result_prototype_index",
    "stealth_target_prototype_indices", "create_craters", "worker_strength",
    "air_defense", "icon_index", "civilopedia_entry",
]
START_FIELDS = [
    "civilization_id", "civilization", "source_biq", "expansionist", "militaristic",
    "starting_unit_1", "starting_unit_1_count", "starting_unit_2",
    "starting_unit_2_count", "expansionist_bonus_unit", "bonus_count",
]


def text(data: bytes) -> str:
    return data.split(b"\0", 1)[0].decode("cp1252").strip()


def section(data: bytes, tag: bytes) -> tuple[int, int]:
    offset = data.find(tag)
    if offset < 0:
        raise ValueError(f"BIQ section {tag!r} not found")
    return offset, struct.unpack_from("<I", data, offset + 4)[0]


def read_fixed_records(data: bytes, tag: bytes, name_offset: int, name_size: int):
    offset, count = section(data, tag)
    cursor = offset + 8
    result = []
    for index in range(count):
        length = struct.unpack_from("<i", data, cursor)[0]
        result.append((index, cursor, length, text(data[cursor + name_offset:cursor + name_offset + name_size])))
        cursor += length + 4
    return result


def normalize(value: str) -> str:
    return re.sub(r"[^a-z0-9]", "", value.casefold())


def unit_id(value: str) -> str:
    result = re.sub(r"[^a-z0-9]+", "_", value.casefold()).strip("_")
    return result.replace("ae_gis", "aegis")


def art_mapping(source: Path, name: str, expansion: str) -> tuple[str, str]:
    folders = ERA_ART.get(name, [ART_ALIASES.get(name, name)])
    roots = [
        ("base", source / "Art/Units"),
        ("ptw", source / "civ3PTW/Art/Units"),
        ("conquests", source / "Conquests/Art/Units"),
    ]
    preferred = UNIQUE_UNITS.get(name, (None, None, None))[2] or expansion
    for folder in folders:
        candidates = sorted(roots, key=lambda entry: entry[0] != preferred)
        if name in UNIQUE_UNITS:
            candidates = [entry for entry in candidates if entry[0] == preferred]
        found = next(
            ((label, path / folder) for label, path in candidates
             if (path / folder).is_dir()),
            None,
        )
        if found is None:
            raise ValueError(f"No source unit art folder for {name!r} ({folder!r})")
        ini_files = [entry for entry in found[1].iterdir() if entry.suffix.casefold() == ".ini"]
        if len(ini_files) != 1:
            raise ValueError(f"Expected one art INI in {found[1]}, found {len(ini_files)}")
    roots_used = []
    for folder in folders:
        candidates = sorted(roots, key=lambda entry: entry[0] != preferred)
        if name in UNIQUE_UNITS:
            candidates = [entry for entry in candidates if entry[0] == preferred]
        match = next((label for label, path in candidates if (path / folder).is_dir()), None)
        roots_used.append(match)
    return "|".join(folders), "|".join(roots_used)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--unpacked-biq", type=Path, required=True)
    parser.add_argument("--source-root", type=Path, default=Path("../source"))
    parser.add_argument("--output", type=Path, default=Path("data/rulesets/civ3_conquests/units.tsv"))
    args = parser.parse_args()
    source = args.source_root.resolve()
    data = args.unpacked_biq.read_bytes()
    biq_path = source / "Conquests/conquests.biq"
    biq_sha256 = hashlib.sha256(biq_path.read_bytes()).hexdigest()

    technologies = {index: name for index, _, _, name in read_fixed_records(data, b"TECH", 4, 32)}
    resources = {index: name for index, _, _, name in read_fixed_records(data, b"GOOD", 4, 24)}

    proto_offset, proto_count = section(data, b"PRTO")
    cursor = proto_offset + 8
    prototypes: dict[str, list[dict[str, object]]] = {}
    for index in range(proto_count):
        length = struct.unpack_from("<i", data, cursor)[0]
        name = text(data[cursor + 8:cursor + 40])
        mask = int.from_bytes(data[cursor + 144:cursor + 148], "little")
        flags_1 = data[cursor + 136:cursor + 144]
        flags_2 = data[cursor + 148:cursor + 156]
        flags_3 = data[cursor + 168:cursor + 188]
        ignore_movement_cost = data[cursor + 192:cursor + 206]
        values = struct.unpack_from("<16i", data, cursor + 72)
        stealth_target_count = struct.unpack_from("<i", data, cursor + 234)[0]
        stealth_target_indices = struct.unpack_from(
            f"<{stealth_target_count}i", data, cursor + 238
        ) if stealth_target_count else ()
        dynamic_tail = cursor + 238 + stealth_target_count * 4
        if dynamic_tail + 21 != cursor + length + 4:
            raise ValueError(f"Unexpected variable PRTO record length for {name!r}")
        # PRTO values through required resources: bombard, range, capacity,
        # cost, defense, icon, attack, op range, pop, ROF, movement, tech,
        # upgrade, and three resource indices.
        row = {
            "index": index,
            "cursor": cursor,
            "length": length,
            "mask": mask,
            "bombard": values[0],
            "bombard_range": values[1],
            "capacity": values[2],
            "cost": values[3],
            "defense": values[4],
            "attack": values[6],
            "op_range": values[7],
            "pop_cost": values[8],
            "rof": values[9],
            "moves": values[10],
            "tech": values[11],
            "upgrade": values[12],
            "resources": values[13:16],
            "type": struct.unpack_from("<i", data, cursor + 156)[0],
            "other_strategy": struct.unpack_from("<i", data, cursor + 160)[0],
            "hp_bonus": struct.unpack_from("<i", data, cursor + 164)[0],
            "flags_1": flags_1,
            "flags_2": flags_2,
            "flags_3": flags_3,
            "ignore_movement_cost": ignore_movement_cost,
            "zone_of_control": struct.unpack_from("<i", data, cursor + 4)[0],
            "icon_index": values[5],
            "civilopedia_entry": text(data[cursor + 40:cursor + 72]),
            "bombard_effects": struct.unpack_from("<i", data, cursor + 188)[0],
            "require_support": struct.unpack_from("<i", data, cursor + 206)[0],
            "enslave_result": struct.unpack_from("<i", data, cursor + 226)[0],
            "stealth_target_indices": stealth_target_indices,
            "create_craters": data[dynamic_tail + 8],
            "worker_strength": struct.unpack_from("<f", data, dynamic_tail + 9)[0],
            "air_defense": struct.unpack_from("<i", data, dynamic_tail + 17)[0],
        }
        prototypes.setdefault(name, []).append(row)
        cursor += length + 4

    names = {name for name, rows in prototypes.items() if any(row["mask"] for row in rows)} | WONDER_UNITS
    if len(names) != 92:
        raise ValueError(f"Expected 92 campaign identities from source filters, found {len(names)}")
    civ_by_index = dict(enumerate(CAMPAIGN_CIVS))
    output = []
    for name in sorted(names, key=lambda item: min(r["index"] for r in prototypes[item])):
        rows = prototypes[name]
        source_row = next((row for row in rows if row["mask"]), rows[0])
        for duplicate in rows:
            ignored = {"index", "cursor", "length", "mask", "flags_1", "other_strategy"}
            if any(duplicate[key] != source_row[key] for key in source_row if key not in ignored):
                raise ValueError(f"Conflicting non-AI unit data in duplicate PRTO records for {name!r}")
            if duplicate["flags_1"][:4] != source_row["flags_1"][:4]:
                raise ValueError(f"Conflicting gameplay PRTO flags in duplicate records for {name!r}")
        # Repeated entries are source prototypes for the same rules identity;
        # preserve every index, common gameplay data, and each source AI profile.
        mask = 0
        for row in rows:
            mask |= int(row["mask"])
        available = [civ_by_index[index] for index in range(1, 32) if mask & (1 << index)]
        if name in UNIQUE_UNITS:
            category = "unique"
            civ, replaces, expansion = UNIQUE_UNITS[name]
        elif name in WONDER_UNITS:
            category, civ, replaces, expansion = "wonder_created", "", "", "conquests"
        else:
            category, civ, replaces, expansion = "standard", "", "", "base"
            if source_row["index"] >= 101:
                expansion = "conquests"
            elif source_row["index"] >= 66:
                expansion = "ptw"
        art_folder, art_root = art_mapping(source, name, expansion)
        row = {
            "id": unit_id(name), "name": name, "category": category, "expansion": expansion,
            "source_prototypes": ";".join(str(item["index"]) for item in rows),
            "source_biq": f"Conquests/conquests.biq#PRTO[{source_row['index']}]@{biq_sha256}",
            "cost_shields": source_row["cost"], "population_cost": source_row["pop_cost"],
            "attack": source_row["attack"], "defense": source_row["defense"], "moves": source_row["moves"],
            "bombard_strength": source_row["bombard"], "bombard_range": source_row["bombard_range"],
            "rate_of_fire": source_row["rof"], "operational_range": source_row["op_range"],
            "capacity": source_row["capacity"],
            "required_technology": technologies.get(source_row["tech"], "") if source_row["tech"] >= 0 else "",
            "required_resource_1": resources.get(source_row["resources"][0], "") if source_row["resources"][0] >= 0 else "",
            "required_resource_2": resources.get(source_row["resources"][1], "") if source_row["resources"][1] >= 0 else "",
            "required_resource_3": resources.get(source_row["resources"][2], "") if source_row["resources"][2] >= 0 else "",
            "upgrade_to": prototypes_by_index_name(prototypes, source_row["upgrade"]),
            "available_to": ";".join(available), "unique_civilization": civ, "replaces": replaces,
            "unit_type": source_row["type"], "flags_1_hex": source_row["flags_1"].hex(),
            "flags_2_hex": source_row["flags_2"].hex(), "flags_3_hex": source_row["flags_3"].hex(),
            "hit_point_bonus": source_row["hp_bonus"], "ai_other_strategy": source_row["other_strategy"],
            "source_ai_profiles": "|".join(
                f"{item['index']}:{item['other_strategy']}:{item['flags_1'].hex()}" for item in rows
            ),
            "art_folder": art_folder, "art_root": art_root,
            "ignore_movement_cost_hex": source_row["ignore_movement_cost"].hex(),
            "zone_of_control": source_row["zone_of_control"],
            "bombard_effects": source_row["bombard_effects"],
            "require_support": source_row["require_support"],
            "enslave_result_prototype_index": source_row["enslave_result"],
            "stealth_target_prototype_indices": ";".join(
                str(index) for index in source_row["stealth_target_indices"]
            ),
            "create_craters": source_row["create_craters"],
            "worker_strength": format(source_row["worker_strength"], ".9g"),
            "air_defense": source_row["air_defense"],
            "icon_index": source_row["icon_index"],
            "civilopedia_entry": source_row["civilopedia_entry"],
        }
        output.append(row)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", newline="", encoding="utf-8") as stream:
        writer = csv.DictWriter(stream, fieldnames=FIELDS, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(output)
    write_start_profiles(data, prototypes, technologies, resources, args.output.parent, biq_sha256)
    write_experience_levels(data, args.output.parent, biq_sha256)
    write_air_mission_settings(data, args.output.parent, biq_sha256)
    print(f"Wrote {len(output)} campaign unit definitions to {args.output}")
    print(f"Unique units: {sum(row['category'] == 'unique' for row in output)}; wonder-created: {sum(row['category'] == 'wonder_created' for row in output)}")


def write_start_profiles(
    data: bytes,
    prototypes: dict[str, list[dict[str, object]]],
    _technologies: dict[int, str],
    _resources: dict[int, str],
    output_dir: Path,
    biq_sha256: str,
) -> None:
    era_count = section(data, b"ERAS")[1]
    race_offset, race_count = section(data, b"RACE")
    cursor = race_offset + 8
    civs = []
    for index in range(race_count):
        length, city_count = struct.unpack_from("<ii", data, cursor)
        leader_count_pos = cursor + 8 + city_count * 24
        leader_count = struct.unpack_from("<i", data, leader_count_pos)[0]
        fixed = leader_count_pos + 4 + leader_count * 32
        source_name = text(data[fixed + 128:fixed + 168])
        noun = text(data[fixed + 168:fixed + 208])
        trait_flags = data[fixed + 208 + era_count * 520 + 52:fixed + 208 + era_count * 520 + 68]
        is_expansionist = bool(trait_flags[0] & (1 << 2))
        is_militaristic = bool(trait_flags[0] & (1 << 0))
        civs.append((index, source_name, noun, is_expansionist, is_militaristic))
        cursor += length + 4

    rule_offset, _ = section(data, b"RULE")
    rule = rule_offset + 8
    spaceship_count = struct.unpack_from("<i", data, rule + 100)[0]
    # RULE's editable fixed record is split around its spaceship and culture
    # arrays. The second fixed block begins after 104 bytes plus ship parts.
    second = rule + 104 + spaceship_count * 4
    culture_count = struct.unpack_from("<i", data, second + 160)[0]
    scout_index = struct.unpack_from("<i", data, second + 108)[0]
    start1_index = struct.unpack_from("<i", data, second + 120)[0]
    start2_index = struct.unpack_from("<i", data, second + 124)[0]
    unit_by_index = {row["index"]: name for name, rows in prototypes.items() for row in rows}
    if culture_count <= 0:
        raise ValueError("RULE culture count was not found where expected")
    default_1 = unit_by_index.get(start1_index)
    default_2 = unit_by_index.get(start2_index)
    scout = unit_by_index.get(scout_index)
    if (default_1, default_2, scout) != ("Settler", "Worker", "Scout"):
        raise ValueError(f"Unexpected Conquests start defaults: {default_1}, {default_2}, scout={scout}")
    display_names = dict(enumerate(CAMPAIGN_CIVS))
    rows = []
    for index, _source_name, _noun, expansionist, militaristic in civs:
        if index == 0:
            continue
        civ_name = display_names[index]
        bonus = ""
        if expansionist:
            bonus = next((
                name for name, (unique_civ, replaces, _) in UNIQUE_UNITS.items()
                if unique_civ == civ_name and replaces == scout
            ), scout)
        rows.append({
            "civilization_id": unit_id(civ_name),
            "civilization": civ_name,
            "source_biq": f"Conquests/conquests.biq#RACE[{index}]+RULE@{biq_sha256}",
            "expansionist": str(expansionist).lower(),
            "militaristic": str(militaristic).lower(),
            "starting_unit_1": default_1, "starting_unit_1_count": 1,
            "starting_unit_2": default_2, "starting_unit_2_count": 1,
            "expansionist_bonus_unit": bonus, "bonus_count": 1 if expansionist else 0,
        })
    if len(rows) != 31 or sum(row["expansionist"] == "true" for row in rows) != 8:
        raise ValueError("Expected 31 playable civilization starts and 8 Expansionist profiles")
    out_path = output_dir / "civilization_starts.tsv"
    with out_path.open("w", newline="", encoding="utf-8") as stream:
        writer = csv.DictWriter(stream, fieldnames=START_FIELDS, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def write_experience_levels(data: bytes, output_dir: Path, biq_sha256: str) -> None:
    fields = [
        "rank_id", "rank_name", "base_hit_points", "retreat_bonus_percent",
        "promotion_chance_per_mille", "default_rank", "source_biq",
        "promotion_chance_source",
    ]
    promotion_chances = [500, 250, 125, 0]
    default_index = 1  # The Conquests RULE defaults units to Regular.
    rows = []
    for index, cursor, _length, name in read_fixed_records(data, b"EXPR", 4, 32):
        if index >= len(promotion_chances):
            raise ValueError(f"Unexpected Conquests experience level {index}: {name}")
        hp, retreat_bonus = struct.unpack_from("<ii", data, cursor + 36)
        rows.append({
            "rank_id": unit_id(name),
            "rank_name": name,
            "base_hit_points": hp,
            "retreat_bonus_percent": retreat_bonus,
            "promotion_chance_per_mille": promotion_chances[index],
            "default_rank": str(index == default_index).lower(),
            "source_biq": f"Conquests/conquests.biq#EXPR[{index}]+RULE@{biq_sha256}",
            "promotion_chance_source": "CivFanatics Civilization III FAQ, Warfare",
        })
    if len(rows) != 4 or sum(row["default_rank"] == "true" for row in rows) != 1:
        raise ValueError("Expected four Conquests experience levels and one default")
    out_path = output_dir / "experience_levels.tsv"
    with out_path.open("w", newline="", encoding="utf-8") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)


def write_air_mission_settings(data: bytes, output_dir: Path, biq_sha256: str) -> None:
    """Export the stock RULE air-interception chances into editable settings."""
    fields = ["key", "value_percent", "source_biq"]
    rule_offset, count = section(data, b"RULE")
    if count != 1:
        raise ValueError(f"Expected one Conquests RULE record, found {count}")
    record = rule_offset + 8
    record_length = struct.unpack_from("<i", data, record)[0]
    spaceship_count = struct.unpack_from("<i", data, record + 100)[0]
    second_block = record + 104 + spaceship_count * 4
    # RULE.cs in the bundled OpenCiv3 BIQ reader places these two fields at
    # offsets 80 and 84 in RULE's second fixed block.
    values = {
        "chance_to_intercept_air_missions": struct.unpack_from("<i", data, second_block + 80)[0],
        "chance_to_intercept_stealth_missions": struct.unpack_from("<i", data, second_block + 84)[0],
    }
    if not 0 <= record_length <= 4096 or any(not 0 <= value <= 100 for value in values.values()):
        raise ValueError(f"Invalid Conquests air mission settings: {values}")
    out_path = output_dir / "air_missions.tsv"
    with out_path.open("w", newline="", encoding="utf-8") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows({
            "key": key,
            "value_percent": value,
            "source_biq": f"Conquests/conquests.biq#RULE[0].{source_field}@{biq_sha256}",
        } for key, value, source_field in [
            ("chance_to_intercept_air_missions", values["chance_to_intercept_air_missions"], "ChanceToInterceptEnemyAirMissions"),
            ("chance_to_intercept_stealth_missions", values["chance_to_intercept_stealth_missions"], "ChanceToInterceptEnemyStealthMissions"),
        ])


def prototypes_by_index_name(prototypes: dict[str, list[dict[str, object]]], index: int) -> str:
    if index < 0:
        return ""
    for name, rows in prototypes.items():
        if any(row["index"] == index for row in rows):
            return name
    return f"PRTO[{index}]"


if __name__ == "__main__":
    main()
