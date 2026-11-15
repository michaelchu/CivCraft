#!/usr/bin/env python3
"""Convert the checked-in Conquests extraction tables to native TOML.

This is intentionally a small, dependency-free migration tool.  The generated
files are the new editable source of truth; the TSV inputs remain available
until the runtime has been switched and the parity checks have stayed green.
"""

from __future__ import annotations

import csv
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "data/rulesets/civ3_conquests"
DEST = ROOT / "data/rulesets/civcraft_native"


def read_tsv(name: str) -> list[dict[str, str]]:
    with (SOURCE / name).open(newline="", encoding="utf-8") as stream:
        return list(csv.DictReader(stream, delimiter="\t"))


def string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def strings(value: str, separator: str = ";") -> str:
    return "[" + ", ".join(string(part) for part in value.split(separator) if part) + "]"


def integers(value: str, separator: str = ";") -> str:
    return "[" + ", ".join(part for part in value.split(separator) if part) + "]"


def write_units() -> None:
    rows = read_tsv("units.tsv")
    fields = [
        ("id", string),
        ("name", string),
        ("category", string),
        ("expansion", string),
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


DEST.mkdir(parents=True, exist_ok=True)
write_units()
write_civilizations()
