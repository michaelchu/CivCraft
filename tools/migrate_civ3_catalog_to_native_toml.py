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
    for raw_line in (ROOT / "data/rulesets/civcraft_civ2civ3/units.ruleset").read_text(
        encoding="utf-8"
    ).splitlines():
        line = raw_line.strip()
        if line.startswith("[unit_") and line.endswith("]"):
            current = {}
            profiles[line[1:-1]] = current
            continue
        if current is None or "=" not in line or line.startswith(";"):
            continue
        key, value = (part.strip() for part in line.split("=", 1))
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
        }:
            match = re.search(r"-?\d+", value)
            if match:
                current[key] = int(match.group())
        elif key in {"flags", "roles", "targets"}:
            current[key] = re.findall(r'"([^"\\]*(?:\\.[^"\\]*)*)"', value)
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
    for profile in profiles.values():
        unit_class = profile.get("unit_class")
        profile["class_flags"] = class_flags.get(str(unit_class), [])
        profile.setdefault("targets", [])
        profile.setdefault("paratroopers_range", -1)
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
    lines = [
        "schema_version = 1",
        'source = "Civilization III: Conquests"',
        "",
    ]
    for row in read_tsv("government_unit_support.tsv"):
        lines.extend(
            [
                "[[governments]]",
                f'government = {string(row["government"])}',
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
        roles = profile.get("roles", [])
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
                f'paratroopers_range = {profile.get("paratroopers_range", -1)}',
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


if __name__ == "__main__":
    DEST.mkdir(parents=True, exist_ok=True)
    write_units()
    write_civilizations()
    write_buildings()
    write_technologies()
    write_governments()
    write_unit_economics()
