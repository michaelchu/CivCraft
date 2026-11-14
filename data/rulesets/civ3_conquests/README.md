# Civ III: Conquests unit data

This directory contains the first source-derived rules inputs for the clean
Conquests unit rebuild. `units.tsv` has one active campaign identity per row;
`civilization_starts.tsv` records the normal campaign starting profile for each
playable civilization.

## Source and reproducibility

The gameplay records come from the installed `source/Conquests/conquests.biq`
`PRTO`, `TECH`, `GOOD`, `RACE`, `ERAS`, and `RULE` sections. The original BIQ is
PKWare DCL-compressed. From `app/`, unpack it with the adjacent reference
utility and regenerate the tables:

```sh
cc -DTEST ../openciv3/Blast/reference-code/blast.c -o /tmp/civ3-blast
/tmp/civ3-blast < ../source/Conquests/conquests.biq > /tmp/conquests.unpacked
python3 tools/export_civ3_conquests_units.py --unpacked-biq /tmp/conquests.unpacked
```

Each `source_biq` cell includes the source section/index and SHA-256 of the
installed BIQ. The exporter requires exactly 92 identities: all distinct PRTO
names with an ordinary civilization availability mask, plus Ancient Cavalry
and Crusader, the two wonder-created units whose masks are empty. Scenario-only
prototypes and ruler/Princess entries are not included.

The BIQ is the target rules data for the installed Conquests edition. The
manual remains the primary gameplay explanation where it covers a mechanic;
the manual predates Play the World and Conquests. The BIQ-derived table is
therefore the authoritative source for expansion values, availability,
replacements, technologies, resources, upgrades, and special flags. No
Freeciv-derived values are used to fill gaps.

## Building and technology source catalogs

`buildings.tsv` and `technologies.tsv` preserve all 83 Conquests `BLDG` and 83
`TECH` records from the installed BIQ. They retain source order, Civilopedia
keys, every gameplay scalar, raw flag bytes, unknown tail bytes, and raw
cross-reference indices. Resolved names are convenience fields; the indices
remain authoritative. Each row records the BIQ path, section, record index,
and source SHA-256. The BLDG flavor-description prose is omitted; the compact
gameplay values and identifiers are retained.

Regenerate these catalogs from the same unpacked BIQ used for the unit export:

```sh
python3 tools/export_civ3_conquests_buildings_techs.py \
  --unpacked-biq /tmp/conquests.unpacked \
  --source-root ../source
```

`building_catalog_bindings.tsv` records the partial crosswalk to the current
serialized `Building` wrappers. It maps 47 of 83 source records. The other 36
remain source-addressable and carry an explicit unmapped reason; do not infer
their identities from Freeciv record order. The crosswalk reuses the existing
culture-export mapping, whose numeric values are legacy CivCraft IDs.

`RulesetData::load_with_civ3_catalog` attaches these Civ III records alongside
the selected Freeciv ruleset. The source records remain distinct from
Freeciv's native content until the owning gameplay subsystem consumes them.
The runtime binding layer currently resolves all 92 units, all 31 playable
civilizations, all 83 technologies, and 47 buildings. `Amphibious War`
explicitly binds to the existing `Amphibious Warfare` technology ID, and
`Ironclads` binds to `Tech::IRONCLADS`. Thirty-six building wrappers remain
unresolved with explicit reasons; their source records are still preserved.
This is an adapter inventory, not a gameplay cutover.

## Unit table

`units.tsv` is UTF-8, tab-separated, and editable in a text editor or spreadsheet.
The source fields are kept as explicit columns:

- `cost_shields`, `population_cost`, combat values, movement, bombardment,
  ranges, capacity, technology, resources, and upgrade target come from PRTO
  records, with names resolved through TECH, GOOD, and PRTO indices.
- `available_to` is the union of PRTO civilization masks mapped by BIQ RACE
  index. It records the masks as stored in the Conquests BIQ; it is not by
  itself a complete answer to whether a civilization can build a normal unit
  when it has a unique replacement.
- `unique_civilization` and `replaces` encode the 31 unique-unit substitutions.
  They are explicit relationships on separate unit identities.
- `flags_1_hex`, `flags_2_hex`, and `flags_3_hex` retain the original BIQ flag
  bytes without lossy renaming. Their field meanings are documented in the
  adjacent OpenCiv3 checkout's `QueryCiv3/BiqSections/Prto.cs`.
- `ignore_movement_cost_hex` retains the PRTO terrain mask for terrain types
  whose movement cost the unit ignores; it is separate from the all-terrain
  road-movement flag and must be interpreted against the Conquests TERR order.
- `zone_of_control`, `bombard_effects`, `require_support`,
  `enslave_result_prototype_index`, `stealth_target_prototype_indices`,
  `create_craters`, `worker_strength`, and `air_defense` preserve additional
  PRTO fields that affect behavior or visual effects. Stealth-target lists and
  enslave results retain their original PRTO prototype indices; these indices
  are source references, not `UnitKind` IDs.
- `icon_index` and `civilopedia_entry` preserve the source unit icon index and
  Civilopedia key. They remain separate from the mapped unit art folder.
- `source_prototypes` lists every same-name BIQ prototype. Some duplicate
  records share all gameplay values but differ in AI strategy fields. The
  representative row uses the first prototype, while `source_ai_profiles`
  preserves each prototype index, strategy value, and complete flags-1 value.
- `art_folder` and `art_root` map the unit identity to the matching original
  folder in `source/Art/Units`, `source/civ3PTW/Art/Units`, or
  `source/Conquests/Art/Units`. `|` separates era-specific Army and Leader
  art variants. The exporter prefers the unit's introduction expansion when
  duplicate folders exist (Conquests Paratrooper uses its Conquests manifest;
  Modern Paratrooper uses the base manifest). The runtime loader resolves
  same-unit shared files across the art roots, expands `.ini` action, timing,
  and speed data, and decodes the referenced FLC animations.

`category` distinguishes standard, unique, and wonder-created identities.
`expansion` labels the unit's introduction: base, Play the World, or Conquests.
These are classification metadata; the Conquests BIQ remains the source of the
installed rules values.

### Unit animation pacing

`unit_animation_pacing.toml` contains presentation timing overrides layered on
top of each unit INI's animation files and source timing values. Work-cycle
duration multipliers above `1.0` slow playback. Settings under `[units.<unit>]`
override the shared defaults, while `[units.<unit>.actions.<action>]` can
override one action for one unit. `[units.<unit>.fidget]` can set the selected
unit's fidget play and rest durations independently. Unit keys use the
lowercase Conquests unit ID from `units.tsv`; action keys use lowercase
animation names such as `road`, `mine`, or `build`.

### Unique-unit availability notes

Keep the raw BIQ availability mask separate from the unique-unit replacement
relationship. In the installed Conquests BIQ, the standard Warrior mask still
includes the Aztecs even though Jaguar Warrior replaces Warrior, and the
standard Cannon mask includes the Koreans even though Hwach'a replaces Cannon.
The other replacement pairs in this export omit their unique civilization
from the standard unit's mask. The two exceptions are retained verbatim in
`available_to`; do not silently normalize the source column. The rules layer
must use `unique_civilization` and `replaces` when resolving the buildable unit
for that civilization. This is a source-data asymmetry, not evidence that the
Aztecs or Koreans should receive both units. The original game executable's
resolution of these two masks has not yet been independently verified, so
keep any runtime interpretation explicit and revisable.

## Starting profiles

The `RULE` record names Settler and Worker as the default starts, one of each.
The `RACE` records identify eight Expansionist civilizations, which receive
one additional Scout. Inca's available Scout identity is its unique
replacement, Chasqui Scout, so the table records that specific unit for the
Inca profile. The exporter checks 31 playable civilizations and eight
Expansionist profiles; the other 23 profiles have no bonus unit.

## Runtime status

The simulation reads the editable table for all 92 active identities, costs,
population costs, attack and defense, movement, technologies, resources,
upgrades, civilization masks, unique-unit relationships, and raw BIQ flags.
Hit points use the source experience-rank table plus each unit's PRTO hit-point
bonus. It also reads the civilization starting profiles. Unit stats and
catalog metadata no longer fall back to the former Freeciv/Civ2 per-unit
switches: every active campaign identity must resolve to a row in `units.tsv`.
The additional PRTO fields listed above are parsed and preserved in the runtime
rule records. `ZoneOfControl` now determines which units block enemy movement;
the distinct Civ III zone reaction-attack behavior still needs implementation.
Ordinary capture of undefended units is derived from combat values because
Conquests PRTO has no `Capturable` bit: units with zero attack and defense can
be captured by attack-capable units when their required technology is known;
captured Settlers become two Workers. Enslavement is separate: on combat
victory, units with a valid `EnslaveResults` PRTO index have a one-in-three
chance to create that result unit at the battle tile under the victor's
control. The created unit retains the defeated unit's nationality, inferred
from the manual's captured-unit nationality rule. See
`docs/CIV3_CONQUESTS_UNIT_FIELD_AUDIT.md` for the source and remaining limits.
Unit `AirDefense` is applied when an aircraft or missile bombards a tile: up to
four defending units independently roll their Air Defense strength against
the bomber's base defense; a successful AA hit destroys the bomber and cancels
the bombardment. On the simulation's combat scale this is
`air_defense / (air_defense + 10 * bomber_defense)`, matching the community
reverse-engineered one-tenth editor-strength formula. This is distinct from
air-superiority fighter patrols and city SAM Battery interception, which are
not implemented here. Bombard presentation effects, stealth-target UI
selection, and worker strength still need behavior-level implementations.

Unit support follows Civ III's treasury model: support-eligible units do not
consume food or shields, and each unit above the empire's free allowance costs
the government rate in gold. Free slots are summed by city size (town 1–6,
city 7–12, metropolis 13+). The rules are editable in
`government_unit_support.tsv`; Settlers and Workers are support-eligible and
their population production costs remain 2 and 1, respectively. See
`docs/CIV3_MANUAL_RULES.md` for source attribution and the remaining government
behavior gaps. The runtime government roster now uses the eight standard
Conquests entries, including Feudalism and Fascism.

Some runtime inputs are not explicit PRTO fields and currently use documented
defaults or derived interpretations:

- Ordinary combat firepower currently defaults to 1; PRTO rate of fire is
  retained for bombardment and is not used as ordinary combat firepower. The
  ordinary firepower default still needs direct Conquests source confirmation.
- Unit vision currently uses squared radius 2; the PRTO export has no
  per-unit vision field. Treat this as a provisional engine default pending
  source-level confirmation, not as verified per-unit parity.
- Unit classes and some cargo/role metadata are inferred from PRTO unit type,
  capacity, and raw flags. Government restrictions and obsolescence do not yet
  have complete editable columns in this table.
- Veteran rank count uses the standard four Conquests ranks for units able to
  gain experience; the rank hit-point values and default rank come from
  `experience_levels.tsv`.

The renderer maps every active identity to original Civ III art, loads its
animation states and timing from the mapped INIs, and uses source art in the
map and unit-selection/production UI. Installed-art coverage is checked by the
`every_campaign_unit_ini_animation_resolves_in_the_installed_art_roots` test.

This is a migration checkpoint, not completion: unit identities such as
Diplomat, Spy, Caravan, and Freight have been removed from runtime, along with
their unit-specific action paths. The former hardcoded stat and metadata
fallbacks have also been removed. Some Freeciv-derived mechanics remain in
shared simulation code, and some Conquests flags and mechanics are not yet
implemented as Civ III rules. These require a broader behavior audit.
# Civilization roster

`civilizations.tsv` contains the 31 playable Conquests `RACE` records in BIQ
order. The first record is Rome; the separate Barbarian record is deliberately
excluded. `source_race_index` preserves the BIQ record index. `source_name`,
`noun`, `adjective`, and `leader` are the extracted source identity fields;
`cities` preserves ordered `RACE_CityName` entries. `primary_color_index` and
`secondary_color_index` correspond to the BIQ `DefaultColor` and `UniqueColor`
palette IDs. `ntp00.pcx` through `ntp31.pcx` in the original source artwork
provide the renderer team-color ramps; CivCraft uses the first ramp color for
the primary tint and uses the secondary ID for the city-banner accent.
`culture_group_index` preserves the raw `RACE.CultureGroup` value; it is
identity metadata and must not be used as accumulated culture points.

OpenCiv3's `ruleset.json` is the checked-in extraction reference, and its BIQ
importer maps those fields from `RACE` and `RaceCityName`; the original
`source/Conquests/conquests.biq` remains authoritative. The `city_style` column
is a CivCraft renderer mapping to one of the five city-sheet styles, not a BIQ
field. Existing map presets still determine how many roster entries participate
in a campaign (4, 6, 8, 12, or 16); the full 31 are available across seeds.

## Culture source export

`culture_biq.json` is a reproducible extraction of culture-related records
from the installed `source/Conquests/conquests.biq`. It preserves the BIQ
SHA-256, section names, and source record indices. It includes all BLDG culture
output, CULT ratio/resistance records, RULE border and culture-level settings,
RACE culture-group indices, GAME cultural-victory and capture-retention fields,
GOVT assimilation and pairwise fields, and all DIFF settings. GOVT pairwise
interpretation and enabled `GAME.RetainCulture` accounting remain unresolved;
the default Conquests capture path interprets its disabled flag as clearing
the captured city's culture ledger. The effective cultural-victory gate is
exported to `culture_victory_settings.tsv` using the community reverse-engineered
rule that default victory conditions enable cultural victory even when its
individual flag is off. See
`docs/CIV3_CULTURE_RULES.md` before using them as gameplay rules.
`culture_capture_settings.tsv` records `GAME.RetainCulture` for all 29 BIQs
installed under `source/Conquests`, retaining each `GAME` record index and BIQ
hash. It is generated by `tools/export_conquests_capture_culture_flags.py`.
`culture_buildings.tsv` is the runtime adapter for culture-positive BLDG rows;
it ties all 46 such records to the CivCraft building catalog while preserving
the BIQ record, hash, and explicit mapping status. Sistine Chapel and Knights
Templar were appended at new IDs with their source prerequisites and
obsolescence metadata.
`culture_victory.tsv` records the Conquests Civilopedia's one-city threshold
and civilization totals for each named map size. Its Standard total is checked
against the BIQ `GAME.AllCitiesCultureWin` value.
`culture_victory_settings.tsv` preserves `GAME.default_victory_conditions`, the
individual cultural-victory flag, their effective result, the `GAME` record
index, and the BIQ hash.

Regenerate it from the installed BIQ with:

```sh
cc -DTEST ../openciv3/Blast/reference-code/blast.c -o /tmp/civ3-blast
/tmp/civ3-blast < ../source/Conquests/conquests.biq > /tmp/conquests.unpacked
python3 tools/export_civ3_conquests_culture.py --unpacked-biq /tmp/conquests.unpacked
python3 tools/export_conquests_capture_culture_flags.py --blast /tmp/civ3-blast
```

The culture exporter also writes `culture_resistance_bands.tsv` from `CULT`,
`government_resistance.tsv` from the ordered `GOVT` pair matrix, and
`government_assimilation.tsv` from the `GOVT.AssimilationChance` values, plus
`resistance_military_law.tsv` from `DIFF`. These tables retain source indices
and the installed BIQ hash; resistance and assimilation use are documented
separately as reverse-engineered, inferred, or Civilopedia-supported.
