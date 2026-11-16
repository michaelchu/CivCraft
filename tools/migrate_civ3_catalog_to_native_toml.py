#!/usr/bin/env python3
"""Convert the checked-in Conquests extraction tables to native TOML.

This is intentionally a small, dependency-free migration tool.  The generated
files are the new editable source of truth; the TSV inputs remain available
until the runtime has been switched and the parity checks have stayed green.
"""

from __future__ import annotations

import csv
import json
import re
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "data/rulesets/civ3_conquests"
DEST = ROOT / "data/rulesets/civcraft_native"


def read_tsv(name: str) -> list[dict[str, str]]:
    with (SOURCE / name).open(newline="", encoding="utf-8") as stream:
        return list(csv.DictReader(stream, delimiter="\t"))


def read_bridge_tsv(name: str) -> list[dict[str, str]]:
    with (ROOT / "data/rulesets/civcraft_civ2civ3" / name).open(
        newline="", encoding="utf-8"
    ) as stream:
        rows = [line for line in stream if line.strip() and not line.startswith("#")]
    return list(csv.DictReader(rows, delimiter="\t"))


def read_c2c3_unit_profiles() -> dict[str, dict[str, object]]:
    profiles: dict[str, dict[str, object]] = {}
    current: dict[str, object] | None = None
    pending_key: str | None = None
    pending_value: list[str] = []

    string_list_keys = {"flags", "roles", "targets", "cargo", "veteran_names"}
    integer_list_keys = {
        "veteran_base_raise_chance",
        "veteran_work_raise_chance",
        "veteran_power_fact",
        "veteran_move_bonus",
    }

    def flush_pending() -> None:
        if current is None or pending_key not in {
            "flags",
            "roles",
            "targets",
            "cargo",
            "veteran_names",
            "veteran_base_raise_chance",
            "veteran_work_raise_chance",
            "veteran_power_fact",
            "veteran_move_bonus",
        }:
            return
        value = " ".join(pending_value)
        if pending_key in string_list_keys:
            current[pending_key] = re.findall(
                r'"([^"\\]*(?:\\.[^"\\]*)*)"', value
            )
        elif pending_key in integer_list_keys:
            current[pending_key] = [int(item) for item in re.findall(r"-?\d+", value)]

    for raw_line in (ROOT / "data/rulesets/civcraft_civ2civ3/units.ruleset").read_text(
        encoding="utf-8"
    ).splitlines():
        line = raw_line.strip()
        if line.startswith("[unit_") and line.endswith("]"):
            flush_pending()
            current = {}
            profiles[line[1:-1]] = current
            pending_key = None
            pending_value = []
            continue
        if current is None or line.startswith(";"):
            continue
        if "=" in line:
            flush_pending()
            pending_key, value = (part.strip() for part in line.split("=", 1))
            pending_value = [value]
        elif pending_key is not None:
            pending_value.append(line)
            continue
        else:
            continue
        key = pending_key
        value = " ".join(pending_value)
        if key in {"class"}:
            match = re.search(r'"([^"]+)"', value)
            if match:
                current["unit_class"] = match.group(1)
        elif key in {
            "uk_food",
            "uk_shield",
            "uk_gold",
            "uk_happy",
            "build_cost",
            "fuel",
            "paratroopers_range",
            "transport_cap",
            "attack",
        }:
            match = re.search(r"-?\d+", value)
            if match:
                current[key] = int(match.group())
        elif key == "tp_defense":
            match = re.search(r'"([^"]+)"', value)
            if match:
                current["transport_defense"] = match.group(1)
        elif key in {"embarks", "disembarks"}:
            match = re.search(r'"([^"]+)"', value)
            if match:
                current[key] = match.group(1)
    flush_pending()
    class_blocks = re.finditer(
        r"\[unitclass_[^\]]+\](.*?)(?=\n\[|\Z)",
        (ROOT / "data/rulesets/civcraft_civ2civ3/units.ruleset").read_text(
            encoding="utf-8"
        ),
        re.DOTALL,
    )
    class_flags: dict[str, list[str]] = {}
    for block_match in class_blocks:
        block = block_match.group(1)
        name_match = re.search(
            r'^name\s*=.*?"(?:\?unitclass:)?([^"]+)"', block, re.MULTILINE
        )
        flags_match = re.search(r"^flags\s*=\s*(.*)", block, re.MULTILINE | re.DOTALL)
        if name_match and flags_match:
            class_flags[name_match.group(1)] = re.findall(
                r'"([^"\\]*(?:\\.[^"\\]*)*)"', flags_match.group(1)
            )
    source = (ROOT / "data/rulesets/civcraft_civ2civ3/units.ruleset").read_text(
        encoding="utf-8"
    )
    global_block = re.search(
        r"\[veteran_system\](.*?)(?=\n\[|\Z)", source, re.DOTALL
    )
    if global_block is None:
        raise ValueError("C2C3 units.ruleset has no global veteran_system")
    global_text = global_block.group(1)
    global_veteran_names = re.findall(
        r'^veteran_names\s*=\s*(.*)$', global_text, re.MULTILINE
    )
    default_veteran_names = re.findall(
        r'"([^"\\]*(?:\\.[^"\\]*)*)"', global_veteran_names[0]
    )
    default_veteran_values: dict[str, list[int]] = {}
    for key in integer_list_keys:
        match = re.search(rf"^{key}\s*=\s*(.*)$", global_text, re.MULTILINE)
        if match is None:
            raise ValueError(f"global veteran_system has no {key}")
        default_veteran_values[key] = [int(item) for item in re.findall(r"-?\d+", match.group(1))]

    for profile_name, profile in profiles.items():
        unit_class = profile.get("unit_class")
        profile["class_flags"] = class_flags.get(str(unit_class), [])
        profile.setdefault("targets", [])
        profile.setdefault("cargo", [])
        profile.setdefault("transport_cap", 0)
        profile.setdefault("transport_defense", "Alight")
        profile.setdefault("embarks", "")
        profile.setdefault("disembarks", "")
        profile.setdefault("paratroopers_range", -1)
        profile.setdefault("veteran_names", default_veteran_names)
        for key, values in default_veteran_values.items():
            profile.setdefault(key, values)
        profile.setdefault("combat_bonuses", [])

    # Combat bonus tables are the only nested unit table used by CivCraft.
    # Preserve them as native rows instead of making the runtime parse the
    # Freeciv ruleset syntax.
    for match in re.finditer(r"^\[(unit_[^\]]+)\]\s*$", source, re.MULTILINE):
        section = match.group(1)
        start = match.end()
        next_section = re.search(r"^\[", source[start:], re.MULTILINE)
        end = start + next_section.start() if next_section else len(source)
        block = source[start:end]
        table = re.search(
            r"^bonuses\s*=\s*\n\s*\{.*?\n(.*?)^\s*\}",
            block,
            re.MULTILINE | re.DOTALL,
        )
        if table is None or section not in profiles:
            continue
        bonuses = []
        for line in table.group(1).splitlines():
            quoted = re.findall(r'"([^"\\]*(?:\\.[^"\\]*)*)"', line)
            value = re.search(r"-?\d+\s*$", line)
            if len(quoted) == 2 and value:
                bonuses.append(
                    {"flag": quoted[0], "kind": quoted[1], "value": int(value.group())}
                )
        profiles[section]["combat_bonuses"] = bonuses
    return profiles


def string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def strings(value: str, separator: str = ";") -> str:
    return "[" + ", ".join(string(part) for part in value.split(separator) if part) + "]"


def integers(value: str, separator: str = ";") -> str:
    return "[" + ", ".join(part for part in value.split(separator) if part) + "]"


def inline_tables(value: list[dict[str, object]]) -> str:
    def encoded(item: dict[str, object]) -> str:
        fields = []
        for key, part in item.items():
            if isinstance(part, str):
                rendered = string(part)
            elif isinstance(part, bool):
                rendered = str(part).lower()
            else:
                rendered = str(part)
            fields.append(f"{key} = {rendered}")
        return "{ " + ", ".join(fields) + " }"

    return "[" + ", ".join(encoded(item) for item in value) + "]"


def requirement_table(block: str, key: str, section: str) -> list[dict[str, object]]:
    match = re.search(
        rf"^{key}\s*=\s*\n\s*\{{(.*?)^\s*\}}",
        block,
        re.MULTILINE | re.DOTALL,
    )
    if not match:
        return []
    header_line, *rows = match.group(1).splitlines()
    columns = re.findall(r'"([^"\\]*(?:\\.[^"\\]*)*)"', header_line)
    requirements: list[dict[str, object]] = []
    for row_line in rows:
        if not row_line.strip() or row_line.lstrip().startswith(";"):
            continue
        tokens = [
            quoted or bare
            for quoted, bare in re.findall(
                r'"([^"\\]*(?:\\.[^"\\]*)*)"|\b(TRUE|FALSE)\b', row_line
            )
        ]
        if len(tokens) < 3 or len(tokens) > len(columns):
            raise ValueError(f"{section} has malformed {key}")
        row = dict(zip(columns, tokens))
        requirements.append(
            {
                "kind": row["type"],
                "name": row["name"],
                "range": row["range"],
                "present": row.get("present", "true").lower() == "true",
                "survives": row.get("survives", "false").lower() == "true",
                "quiet": row.get("quiet", "false").lower() == "true",
            }
        )
    return requirements


def write_units() -> None:
    rows = read_tsv("units.tsv")
    fields = [
        ("id", string),
        ("name", string),
        ("category", string),
        ("expansion", string),
        ("source_biq", string),
        ("source_prototypes", integers),
        ("cost_shields", int),
        ("population_cost", int),
        ("attack", int),
        ("defense", int),
        ("moves", int),
        ("bombard_strength", int),
        ("bombard_range", int),
        ("rate_of_fire", int),
        ("operational_range", int),
        ("capacity", int),
        ("required_technology", string),
        ("required_resources", strings),
        ("upgrade_to", string),
        ("available_to", strings),
        ("unique_civilization", string),
        ("replaces", string),
        ("unit_type", int),
        ("flags_1_hex", string),
        ("flags_2_hex", string),
        ("flags_3_hex", string),
        ("hit_point_bonus", int),
        ("ai_other_strategy", int),
        ("source_ai_profiles", string),
        ("art_folder", string),
        ("art_root", string),
        ("ignore_movement_cost_hex", string),
        ("zone_of_control", int),
        ("bombard_effects", int),
        ("require_support", int),
        ("enslave_result_prototype_index", int),
        ("stealth_target_prototype_indices", integers),
        ("create_craters", int),
        ("worker_strength", float),
        ("air_defense", int),
        ("icon_index", int),
        ("civilopedia_entry", string),
    ]
    lines = [
        'schema_version = 1',
        'source = "Civilization III: Conquests"',
        "",
    ]
    for row in rows:
        lines.append("[[units]]")
        for key, encode in fields:
            if key == "required_resources":
                value = [
                    row["required_resource_1"],
                    row["required_resource_2"],
                    row["required_resource_3"],
                ]
                lines.append(f"{key} = {[part for part in value if part]!r}".replace("'", '"'))
            else:
                lines.append(f"{key} = {encode(row[key])}")
        lines.append("")
    (DEST / "units.toml").write_text("\n".join(lines), encoding="utf-8")


def write_civilizations() -> None:
    rows = read_tsv("civilizations.tsv")
    starts = {row["civilization"]: row for row in read_tsv("civilization_starts.tsv")}
    lines = [
        'schema_version = 1',
        'source = "Civilization III: Conquests"',
        "",
    ]
    for row in rows:
        start = next(
            (
                candidate
                for candidate in starts.values()
                if candidate["civilization"].lower().startswith(row["noun"].lower())
                or row["noun"].lower().startswith(candidate["civilization"].lower())
            ),
            None,
        )
        if start is None:
            raise KeyError(row["noun"])
        lines.extend(
            [
                "[[civilizations]]",
                f'id = {string(row["id"])}',
                f'source_id = {string(start["civilization_id"])}',
                f'source_biq = {string(start["source_biq"])}',
                f'name = {string(row["name"])}',
                f'source_name = {string(row["source_name"])}',
                f'noun = {string(row["noun"])}',
                f'adjective = {string(row["adjective"])}',
                f'leader = {string(row["leader"])}',
                f'primary_color_index = {row["primary_color_index"]}',
                f'secondary_color_index = {row["secondary_color_index"]}',
                f'city_style = {row["city_style"]}',
                f'starting_techs = [{string(row["starting_tech_1"])}, {string(row["starting_tech_2"])}]',
                f'legacy_names = {strings(row["legacy_names"])}',
                f'source_race_index = {row["source_race_index"]}',
                f'cities = {strings(row["cities"], "|")}',
                f'culture_group_index = {row["culture_group_index"]}',
                f'expansionist = {start["expansionist"].lower()}',
                f'militaristic = {start["militaristic"].lower()}',
                "",
                "[[civilizations.starting_units]]",
                f'id = {string(start["starting_unit_1"])}',
                f'count = {start["starting_unit_1_count"]}',
                "",
                "[[civilizations.starting_units]]",
                f'id = {string(start["starting_unit_2"])}',
                f'count = {start["starting_unit_2_count"]}',
                "",
            ]
        )
        if start["expansionist_bonus_unit"]:
            lines.extend(
                [
                    "[[civilizations.bonus_units]]",
                    f'id = {string(start["expansionist_bonus_unit"])}',
                    f'count = {start["bonus_count"]}',
                    "",
                ]
            )
    (DEST / "civilizations.toml").write_text("\n".join(lines), encoding="utf-8")


def read_native_building_rules() -> list[dict[str, object]]:
    path = DEST / "buildings.toml"
    if not path.is_file():
        raise RuntimeError("native building rules are missing; create buildings.toml first")
    with path.open("rb") as stream:
        rules = tomllib.load(stream).get("rules", [])
    if len(rules) != 91:
        raise RuntimeError(f"expected 91 native building rules, found {len(rules)}")
    return rules


def write_buildings() -> None:
    rows = read_tsv("buildings.tsv")
    bindings = read_tsv("building_catalog_bindings.tsv")
    integer_fields = [
        "source_record_index",
        "doubles_happiness",
        "gain_in_every_city",
        "gain_in_every_city_on_continent",
        "required_building_index",
        "cost",
        "culture",
        "bombard_defense",
        "naval_bombard_defense",
        "defense_bonus",
        "naval_defense_bonus",
        "maintenance_cost",
        "content_faces_all_cities",
        "content_faces",
        "unhappy_faces_all_cities",
        "unhappy_faces",
        "number_of_required_buildings",
        "air_power",
        "naval_power",
        "pollution",
        "production",
        "required_government",
        "spaceship_part",
        "required_advance_index",
        "rendered_obsolete_by_advance_index",
        "required_resource_1_index",
        "required_resource_2_index",
        "number_of_armies_required",
        "flavors",
        "unit_produced_index",
        "unit_frequency",
    ]
    string_fields = [
        "id",
        "source_biq",
        "name",
        "civilopedia_entry",
        "flags_hex",
        "unknown_tail_hex",
        "required_building_name",
        "required_advance_name",
        "rendered_obsolete_by_advance_name",
        "required_resource_1_name",
        "required_resource_2_name",
        "unit_produced_name",
    ]
    lines = [
        "schema_version = 1",
        'source = "Civilization III: Conquests"',
        "",
    ]
    for row in rows:
        lines.append("[[buildings]]")
        for field in string_fields:
            lines.append(f"{field} = {string(row[field])}")
        for field in integer_fields:
            lines.append(f"{field} = {row[field]}")
        lines.append("")
    for row in bindings:
        lines.append("[[bindings]]")
        lines.append(f'civ3_id = {string(row["civ3_id"])}')
        lines.append(f'source_record_index = {row["source_record_index"]}')
        legacy_index = row["legacy_runtime_index"]
        lines.append(f"legacy_runtime_index = {legacy_index or -1}")
        lines.append(f'mapping_status = {string(row["mapping_status"])}')
        lines.append(f'mapping_note = {string(row["mapping_note"])}')
        lines.append("")
    for rule in read_native_building_rules():
        lines.extend(
            [
                "[[rules]]",
                f'legacy_runtime_index = {rule["legacy_runtime_index"]}',
                f'name = {string(rule["name"])}',
                f'genus = {string(rule["genus"])}',
                f'cost = {rule["cost"]}',
                f'upkeep = {rule["upkeep"]}',
                f'requirements = {inline_tables(rule["requirements"])}',
                f'obsolete = {inline_tables(rule["obsolete"])}',
                "",
            ]
        )
    (DEST / "buildings.toml").write_text("\n".join(lines), encoding="utf-8")


def write_technologies() -> None:
    rows = read_tsv("technologies.tsv")
    integer_fields = [
        "source_record_index",
        "cost",
        "era",
        "advance_icon",
        "x",
        "y",
        "prerequisite_1_index",
        "prerequisite_2_index",
        "prerequisite_3_index",
        "prerequisite_4_index",
        "flavors",
    ]
    string_fields = [
        "id",
        "source_biq",
        "name",
        "civilopedia_entry",
        "flags_hex",
        "unknown_tail_hex",
        "prerequisite_1_name",
        "prerequisite_2_name",
        "prerequisite_3_name",
        "prerequisite_4_name",
    ]
    lines = [
        "schema_version = 1",
        'source = "Civilization III: Conquests"',
        "",
    ]
    for row in rows:
        lines.append("[[technologies]]")
        for field in string_fields:
            lines.append(f"{field} = {string(row[field])}")
        for field in integer_fields:
            lines.append(f"{field} = {row[field]}")
        lines.append(
            "prerequisites = "
            + "["
            + ", ".join(
                string(row[f"prerequisite_{index}_name"])
                for index in range(1, 5)
                if row[f"prerequisite_{index}_name"]
            )
            + "]"
        )
        lines.append("")
    (DEST / "technologies.toml").write_text("\n".join(lines), encoding="utf-8")


def write_governments() -> None:
    government_rules = {
        "anarchy": ("Anarchy", "", None, [], 99, 0, 100),
        "despotism": ("Despotism", "", "monarchy", ["ShieldToGold"], 80, 10, 60),
        "monarchy": ("Monarchy", "Monarchy", None, ["ShieldToGold"], 70, 14, 70),
        "communism": ("Communism", "Communism", None, [], 50, 28, 80),
        "republic": ("Republic", "The Republic", None, [], 40, 16, 80),
        "democracy": ("Democracy", "Democracy", None, ["ShieldToGold"], 30, 32, 90),
        "fascism": ("Fascism", "Fascism", None, [], 60, 20, 70),
        "feudalism": ("Feudalism", "Feudalism", None, [], 20, 14, 60),
    }
    lines = [
        "schema_version = 1",
        'source = "Civilization III: Conquests"',
        "",
    ]
    for row in read_tsv("government_unit_support.tsv"):
        name, required, preferred, flags, civil_war, empire_size, max_rates = government_rules[
            row["government"].lower()
        ]
        lines.extend(
            [
                "[[governments]]",
                f'government = {string(row["government"])}',
                f"name = {string(name)}",
                f"required_technology = {string(required)}",
                *([f"preferred_government = {string(preferred)}"] if preferred else []),
                f"flags = {json.dumps(flags)}",
                f"civil_war_chance = {civil_war}",
                f"empire_size_base = {empire_size}",
                f"max_rates = {max_rates}",
                f'town_free_units = {row["town_free_units"]}',
                f'city_free_units = {row["city_free_units"]}',
                f'metropolis_free_units = {row["metropolis_free_units"]}',
                f'cost_per_excess_unit_gold = {row["cost_per_excess_unit_gold"]}',
                "",
            ]
        )
    for row in read_tsv("government_assimilation.tsv"):
        lines.extend(
            [
                "[[assimilation]]",
                f'source_section = {string(row["source_section"])}',
                f'source_record_index = {row["source_record_index"]}',
                f'government = {string(row["government"])}',
                f'assimilation_chance_percent = {row["assimilation_chance_percent"]}',
                f'biq_sha256 = {string(row["biq_sha256"])}',
                "",
            ]
        )
    for row in read_tsv("government_resistance.tsv"):
        lines.extend(
            [
                "[[resistance]]",
                f'captor_biq_index = {row["captor_biq_index"]}',
                f'captor_name = {string(row["captor_name"])}',
                f'defender_biq_index = {row["defender_biq_index"]}',
                f'defender_name = {string(row["defender_name"])}',
                f'modifier = {row["modifier"]}',
                f'biq_sha256 = {string(row["biq_sha256"])}',
                "",
            ]
        )
    for row in read_tsv("resistance_military_law.tsv"):
        lines.extend(
            [
                "[[military_law]]",
                f'source_section = {string(row["source_section"])}',
                f'source_record_index = {row["source_record_index"]}',
                f'difficulty = {string(row["difficulty"])}',
                f'military_law = {row["military_law"]}',
                f'biq_sha256 = {string(row["biq_sha256"])}',
                "",
            ]
        )
    (DEST / "governments.toml").write_text("\n".join(lines), encoding="utf-8")


def write_unit_economics() -> None:
    profiles = read_c2c3_unit_profiles()
    lines = [
        "schema_version = 1",
        'source = "CivCraft native unit-economics bindings"',
        "",
    ]
    for row in read_bridge_tsv("unit_economics.tsv"):
        profile = profiles.get(row["c2c3_record"], {})
        flags = profile.get("flags", [])
        # These records expose ferry transport in the imported C2C3 ruleset,
        # but they are not part of Civ III's native transport roster. Keep
        # the native migration explicit rather than letting the bridge TSV
        # silently grant them a gameplay role.
        roles = (
            []
            if row["civ3_unit_id"].lower() in {"carrack", "dromon", "curragh"}
            else profile.get("roles", [])
        )
        lines.extend(
            [
                "[[bindings]]",
                f'civ3_unit_id = {string(row["civ3_unit_id"])}',
                f'profile = {string(row["c2c3_record"])}',
                f'mapping_kind = {string(row["mapping_kind"])}',
                f'basis = {string(row["basis"])}',
                f'unit_class = {string(profile.get("unit_class", ""))}',
                f'class_flags = {json.dumps(profile.get("class_flags", []), ensure_ascii=False)}',
                f'targets = {json.dumps(profile.get("targets", []), ensure_ascii=False)}',
                f'transport_capacity = {profile.get("transport_cap", 0)}',
                f'cargo = {json.dumps(profile.get("cargo", []), ensure_ascii=False)}',
                f'transport_defense = {string(profile.get("transport_defense", "Alight"))}',
                f'embarks = {string(profile.get("embarks", ""))}',
                f'disembarks = {string(profile.get("disembarks", ""))}',
                f'paratroopers_range = {profile.get("paratroopers_range", -1)}',
                f'attack = {profile.get("attack", 0)}',
                f'veteran_names = {json.dumps(profile.get("veteran_names", []), ensure_ascii=False)}',
                f'veteran_base_raise_chance = {json.dumps(profile.get("veteran_base_raise_chance", []))}',
                f'veteran_work_raise_chance = {json.dumps(profile.get("veteran_work_raise_chance", []))}',
                f'veteran_power_factor = {json.dumps(profile.get("veteran_power_fact", []))}',
                f'veteran_move_bonus = {json.dumps(profile.get("veteran_move_bonus", []))}',
                f'combat_bonuses = {inline_tables(profile.get("combat_bonuses", []))}',
                f'build_cost = {profile.get("build_cost", 0)}',
                f'fuel = {profile.get("fuel", 0)}',
                f'support_food = {profile.get("uk_food", 0)}',
                f'support_shields = {profile.get("uk_shield", 0)}',
                f'support_gold = {profile.get("uk_gold", 0)}',
                f'happiness = {profile.get("uk_happy", 0)}',
                f'flags = {json.dumps(flags, ensure_ascii=False)}',
                f'roles = {json.dumps(roles, ensure_ascii=False)}',
                "",
            ]
        )
    (DEST / "unit_economics.toml").write_text("\n".join(lines), encoding="utf-8")


def write_effects() -> None:
    """Transcribe the selected effect graph into an editable native catalog."""
    source = (ROOT / "data/rulesets/civcraft_civ2civ3/effects.ruleset").read_text(
        encoding="utf-8"
    )
    lines = [
        "schema_version = 1",
        'source = "CivCraft native effect graph"',
        "",
    ]
    sections = re.finditer(
        r"^\[(effect_[^\]]+)\]\s*\n(.*?)(?=^\[|\Z)",
        source,
        re.MULTILINE | re.DOTALL,
    )
    for match in sections:
        section, block = match.groups()
        type_match = re.search(r"^type\s*=\s*\"([^\"]+)\"", block, re.MULTILINE)
        value_match = re.search(r"^value\s*=\s*(-?\d+)", block, re.MULTILINE)
        if not type_match or not value_match:
            raise ValueError(f"effect {section} is missing type or value")
        requirements = requirement_table(block, "reqs", section)
        multiplier_match = re.search(
            r"^multiplier\s*=\s*\"([^\"]+)\"", block, re.MULTILINE
        )
        effect_lines = [
            "[[effects]]",
            f'id = {string(section.removeprefix("effect_"))}',
            f'kind = {string(type_match.group(1))}',
            f"value = {value_match.group(1)}",
        ]
        if multiplier_match:
            effect_lines.append(f"multiplier = {string(multiplier_match.group(1))}")
        effect_lines.extend(
            [f"requirements = {inline_tables(requirements)}", ""]
        )
        lines.extend(effect_lines)
    (DEST / "effects.toml").write_text("\n".join(lines), encoding="utf-8")


def write_actions() -> None:
    source = (ROOT / "data/rulesets/civcraft_civ2civ3/actions.ruleset").read_text(
        encoding="utf-8"
    )
    lines = [
        "schema_version = 1",
        'source = "CivCraft native action graph"',
        "",
    ]
    action_sections = re.finditer(
        r"^\[(action_[^\]]+)\]\s*\n(.*?)(?=^\[|\Z)",
        source,
        re.MULTILINE | re.DOTALL,
    )
    for match in action_sections:
        section, block = match.groups()
        action = re.search(r"^action\s*=\s*\"([^\"]+)\"", block, re.MULTILINE)
        if not action:
            raise ValueError(f"{section} is missing action")
        lines.extend(
            [
                "[[actions]]",
                f'id = {string(section.removeprefix("action_"))}',
                f'action = {string(action.group(1))}',
                "",
            ]
        )
    enabler_sections = re.finditer(
        r"^\[(enabler_[^\]]+)\]\s*\n(.*?)(?=^\[|\Z)",
        source,
        re.MULTILINE | re.DOTALL,
    )
    for match in enabler_sections:
        section, block = match.groups()
        action = re.search(r"^action\s*=\s*\"([^\"]+)\"", block, re.MULTILINE)
        if not action:
            raise ValueError(f"{section} is missing action")
        actor = requirement_table(block, "actor_reqs", section)
        target = requirement_table(block, "target_reqs", section)
        lines.extend(
            [
                "[[enablers]]",
                f'id = {string(section.removeprefix("enabler_"))}',
                f'action = {string(action.group(1))}',
                f"actor_requirements = {inline_tables(actor)}",
                f"target_requirements = {inline_tables(target)}",
                "",
            ]
        )
    (DEST / "actions.toml").write_text("\n".join(lines), encoding="utf-8")


if __name__ == "__main__":
    DEST.mkdir(parents=True, exist_ok=True)
    write_units()
    write_civilizations()
    write_buildings()
    write_technologies()
    write_governments()
    write_unit_economics()
    write_effects()
    write_actions()
