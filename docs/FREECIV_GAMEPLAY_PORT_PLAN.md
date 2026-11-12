# Freeciv gameplay port plan

## Objective

Evolve CivCraft into a Rust implementation of Freeciv gameplay using one
Freeciv ruleset baseline and Civ III content and artwork. The ruleset and engine
behavior are the gameplay authority; Civ III source data remains authoritative
for the content explicitly retained below.

The initial and only Freeciv ruleset baseline for this effort is
`data/civ2civ3/`. Do not mix in `classic` or another Freeciv ruleset to fill
gaps. Create and maintain a CivCraft-owned ruleset derived from that baseline
so changes can be tuned, reviewed, and authored without editing the reference
checkout.

## Authority and preserved content

| Concern | Authority |
|---|---|
| Gameplay algorithms and rule interpretation | Freeciv source at the pinned revision below |
| Initial settings, effects, requirements, actions, and defaults | A CivCraft-owned copy/derivative of the pinned `civ2civ3` ruleset |
| Units and their campaign identities, values, availability, and special data | Civ III: Conquests source catalog, including `data/rulesets/civ3_conquests/units.tsv` |
| Buildings and their campaign identities and source values | Civ III source-derived building catalog; create or reconcile the input catalog where needed |
| Technology tree and advance identities | Civ III source-derived technology catalog in `src/rules/research.rs`, with source gaps recorded explicitly |
| Original unit, terrain, resource, and other selected game art | Civ III source assets and existing asset mappings |

Freeciv ruleset fields still provide the schema through which retained content
interacts with mechanics. Map Civ III records into that schema; do not replace
them with Freeciv's unit, building, or technology content. If a required
mechanic value is absent from Civ III content, take it from the pinned
`civ2civ3` ruleset as an explicit Freeciv-derived setting, not as a claimed
Civ III value.

## Pinned reference

- Freeciv checkout: `../freeciv`
- Revision observed when this plan was written:
  `9ff184b6eb128ca636c7ad6269e27aa959600006`
- Ruleset: `../freeciv/data/civ2civ3/`
- Engine implementation: principally `../freeciv/common/` and
  `../freeciv/server/`
- Ruleset files in the pinned checkout: `actions.ruleset`,
  `buildings.ruleset`, `cities.ruleset`, `effects.ruleset`, `game.ruleset`,
  `governments.ruleset`, `nations.ruleset`, `styles.ruleset`,
  `techs.ruleset`, `terrain.ruleset`, and `units.ruleset`.
- Effective Lua hooks also include `data/default/default.lua`, inherited by
  the ruleset: hut enter/frighten, city transfer (partisans), disaster
  notification, and unit-loss notification. `civ2civ3/parser.lua` is an
  optional Lua helper for `luadata.txt`; it does not parse `.ruleset` files.
  `civ2civ3/script.lua` connects `city_destroyed` (creates Ruins and removes
  Maglev and Railroad) and `map_generated` (places terrain labels using game
  randomness). Freeciv resolves missing ruleset files by whole file through
  the `data/default/` fallback; it does not merge missing sections or values
  into a selected ruleset file. These observable hooks and lookup semantics
  must be accounted for in the Rust port; their exact implementation strategy
  is part of the ruleset/script compatibility design.

When updating the reference revision, review the ruleset and source changes,
record the new full commit here, and refresh affected behavior comparisons.
The reference checkout is for implementation behavior; it does not supersede
the Civ III content sources above.

## Architecture constraints

The port must fit CivCraft's existing Rust organization rather than turn the
simulation into a monolith or mirror Freeciv's C directory tree mechanically.

- Keep rule definitions and catalogs under `src/rules/` and
  `data/rulesets/`; keep state transitions in the owning `src/sim/` subsystem.
- Preserve the `Simulation` authority, `Command`/`ClientSimulation` boundary,
  client view types, and presentation layer. Gameplay rules must not move into
  UI code.
- Add shared ruleset concepts in small cohesive modules (for example,
  requirements, effects, actions, and settings). Subsystems should query those
  shared services instead of reimplementing effect or requirement evaluation.
- Keep one authoritative implementation for each calculation. Remove or route
  duplicate forecast, validation, and execution paths through shared logic
  where they currently risk drifting.
- Keep subsystem logic modular. Extend the existing city, movement, combat,
  research, diplomacy, and AI modules instead of creating a single rules engine
  file. Split modules when a responsibility becomes cohesive and independently
  understandable.
- Preserve explicit ordering, stable IDs, deterministic iteration, and random
  stream boundaries where Freeciv behavior depends on them.
- Keep Civ III source-specific content mapping at the data/adapter boundary;
  do not scatter unit or building identity checks through unrelated systems.
- Keep existing saves usable where practical. Any persistent state change must
  include an intentional save-version and migration decision.

## Work plan and status

Status values: **Not started**, **In progress**, **Complete**, **Blocked**.
Update the status cell and evidence as each item is finished. Complete means
the listed deliverable and its acceptance criteria are satisfied, not merely
that code has been added.

| ID | Work item | Deliverable / completion evidence | Status |
|---|---|---|---|
| P0 | Freeze scope and reference | Single-player campaign scope assumed; Freeciv revision and sole `civ2civ3` baseline pinned below. Inventory Lua/script hooks and ruleset files actually used. | Complete |
| P1 | Map current systems to Freeciv | Code-based subsystem matrix links Rust paths to Freeciv functions and ruleset inputs, records current divergences, and orders migration dependencies. | Complete |
| P2 | Define CivCraft ruleset data model | Rust types and data loading for the Freeciv rule concepts needed by the campaign: settings, requirements, effects, actions/enablers, and content records. Requirements/effects have one shared evaluator. | Complete |
| P3 | Create the CivCraft ruleset baseline | CivCraft-owned editable derivative of `civ2civ3`; provenance and changes recorded; no implicit fallback to another Freeciv ruleset. | Complete |
| P4 | Map Civ III content | Civ III units, buildings, and technologies load through the ruleset model while preserving source identities and values. Missing mappings and unsupported source fields are explicit. | Complete |
| P5 | Port world, turn, visibility, and random behavior | Rust transitions match the pinned Freeciv campaign behavior for the selected ruleset, within CivCraft's map topology and client architecture. | Complete |
| P6 | Port movement, pathfinding, and unit actions | Ruleset-driven movement legality, costs, transport, exploration, worker actions, and action enablers; no parallel hardcoded rule paths remain for migrated behavior. | Not started |
| P7 | Port cities and economy | Ruleset-driven tile output, citizen mood/assignment, growth, production, upkeep, trade allocation, and city management. | Not started |
| P8 | Port research, government, and technology trade | Research progression, government effects, upkeep, revolution, and tech exchange use the shared ruleset model. | Not started |
| P9 | Port combat and military actions | Defender choice, combat, capture, bombardment, aviation, nuclear actions, and veterancy match Freeciv behavior under the CivCraft content mapping. | Not started |
| P10 | Port diplomacy and culture | Treaties, contact, espionage/city actions, culture, borders, migration, conversions, and victory use the Freeciv model and `civ2civ3` settings/effects. This replaces the custom culture path unless a specific CivCraft extension is approved and documented. | Not started |
| P11 | Port AI | City, tax, research, diplomacy, settlement, worker, and military planning use Freeciv behavior adapted to CivCraft state and the Civ III content catalog. | Not started |
| P12 | Port remaining campaign systems | Disasters, climate, villages, spaceship, score, achievements, and endgame behavior match the selected baseline where in campaign scope. | Not started |
| P13 | Validate and cut over | Deterministic comparisons cover every migrated subsystem; end-to-end campaign scenarios cover interactions; old divergent implementations are removed or clearly isolated as unsupported. | Not started |
| P14 | Maintain the port | Document how to change the CivCraft ruleset, add a content mapping, update the pinned Freeciv reference, and extend parity fixtures. | Not started |

### P2 evidence

- Added a native section-file parser, recursive include loader, whole-file
  `default/` fallback, and ordered typed/raw record model in `src/rules/`.
  Repeated sections merge as they do in Freeciv, and duplicate keys after
  include expansion fail with source context. Missing `rule_name` values
  follow Freeciv's untranslated `name` fallback, including translation
  qualifier removal; records without a `name` retain their section identifier.
  `MapGenerationSettings` validates `landmass` and `startpos` and reads the
  C2C3 lake, ocean, and deep-ocean depth properties from typed terrain records.
- Engine identifier tables are copied from the pinned revision's generated
  enum definitions: 68 requirement kinds, 144 actions, and 151 effect types.
  Unknown requirement kinds, action/effect identifiers, and invalid requirement
  values fail loading. Value checks resolve record references against the
  loaded rule names, flags against engine enums and ruleset control tables, and
  numeric values against Freeciv's bounds. The 1,609 requirements in the
  pinned C2C3 include tree use 42 universal kinds; each is validated. A
  recognized kind with no CivCraft runtime resolver returns an explicit
  evaluation error, so it cannot silently behave as absent. Effect and action
  queries also reject unknown identifiers.
- `cargo test --offline rules::freeciv`: 19 passed, including a packaged-data
  integration load. A direct load of the
  pinned `civ2civ3` ruleset parsed 25 settings, 214 parameters, 508 effects,
  19 actions, 89 enablers, 57 units, 73 buildings, 87 technologies, 14
  terrains, 9 governments, 572 nations, 6 styles, and 138 other records.
  The CivCraft-owned derivative adds seven explicit settings (`victories`,
  `landmass`, `startpos`, `steepness`, `wetness`, `specials`, and `huts`), so it
  loads 32 settings.
- Pre-P5 full-suite snapshot, `cargo test --offline --quiet`: 698/703 passed.
  The five failures were
  three map-generation checks (`default_worlds_match_civcraft_resource_goldens`,
  `generated_worlds_match_civcraft_resource_goldens_for_each_wrap`, and
  `civ3_landform_choices_are_deterministic_and_change_landmass_scale`) and two
  endgame scenario setup checks (`multicultural_achievement_counts_nationalities_across_owned_cities`
  and `resistance_requires_loyal_population_and_world_technology`). These paths
  do not call the P2 ruleset model. No before-change full-suite baseline was
  captured, so these failures were not labeled pre-existing. After current P5
  changes, `cargo test --offline --quiet` passes 751/751; none of the five
  historical failures reproduced.

### P3 evidence

- Created `data/rulesets/civcraft_civ2civ3/` from only the pinned C2C3 ruleset,
  with its recursive shared nation include tree under sibling `default/`,
  `nation/`, and `override/` paths. The eleven required gameplay ruleset files
  are all selected from the CivCraft directory; no top-level ruleset falls back
  to `classic` or another Freeciv ruleset.
- Preserved `script.lua`, inherited `default.lua`, `README.civ2civ3`, and the
  upstream `COPYING` text. The Lua files are reference material for future Rust
  ports. The optional `parser.lua` was not copied because C2C3 has no
  `luadata.txt`.
- Added `UPSTREAM_MANIFEST.tsv` with source and copied SHA-256 values for 589
  files: 588 identical files and one modified `game.ruleset`. Its recorded
  changes point the ruledit description to
  `civcraft_civ2civ3/README.civ2civ3` and make the inherited engine defaults
  `victories`, `landmass`, `startpos`, `steepness`, `wetness`, `specials`, and
  `huts` explicit in the editable ruleset.
  `README.md` records their provenance, the source revision, copied
  dependencies, and distribution licensing diligence.
- `packaged_baseline_loads_only_selected_civcraft_ruleset_files` confirms all
  eleven gameplay files resolve from the selected CivCraft ruleset and retain
  the pinned C2C3 record counts.

### P4 evidence

- Added `tools/export_civ3_conquests_buildings_techs.py` and exported all 83
  `BLDG` and 83 `TECH` records from the installed Conquests BIQ
  (`Conquests/conquests.biq`, SHA-256
  `1bd610ab1420217f7d64a9214b60c0bc7686e4781a0d6e66590171535495609c`). The
  building export's names, costs, and culture values match all 83 existing
  culture-BIQ rows. The new tables retain raw BIQ indexes, scalars, flags,
  unknown tails, and provenance. Flavor description prose is omitted.
- `RulesetData::load_with_civ3_catalog` now attaches the 92 unit, 83 building,
  83 technology, and 31 playable-civilization records from the Civ III catalog
  without replacing or reordering the Freeciv baseline records. Each source
  record retains its catalog ID, section/index, source path/hash, and original
  TSV fields.
- Added a stable-ID adapter that resolves all 92 units and all 31
  civilizations, plus 82 of 83 technologies and 47 of 83 buildings. The
  `Amphibious War` to `Amphibious Warfare` alias is explicit. `Ironclads` and
  the remaining 36 building wrapper mappings have explicit unresolved reasons
  in the adapter data; their Civ III records remain available to later
  subsystem ports. No `UnitKind`, `Building`, or `Tech` serialized ordering
  changed.
- The focused adapter test passes and checks catalog counts, source identities,
  raw values, stable wrapper IDs, and unresolved mappings. Catalog loading and
  identity resolution are complete for this milestone. The behavior adapters
  that consume these records belong to P6, P7, and P8, where movement/combat,
  buildings/economy, and research are ported through the shared model.

### P5 evidence

- Turn/calendar ordering now follows the pinned `end_phase` and `begin_phase` paths. Calendar effects, victory flags, year-zero behavior, global technology history, and required save state are covered by deterministic tests. The save header advanced to `\x19`; earlier save formats remain unsupported.
- The default C2C3 FRACTAL path now includes the source-derived terrain, temperature, river, resource, hut, starting-site, map-retry, and map-label RNG steps. Topology and content adaptations remain explicit; the raw Freeciv oracle fixture was not rewritten to match CivCraft output.
- Visibility is ruleset-driven through shared effect/requirement evaluation. All 92 runtime unit IDs have explicit sight bindings or unresolved fallback entries, with unresolved candidates documented.
- Random-state parity intentionally stops before Freeciv-only animal creation: C2C3's `animals=14` creates `Storm` units with no Civ III catalog identity. This preserves the agreed Civ III unit catalog and is recorded as a content adaptation. C2C3 map-label text is likewise not persisted or rendered in this gameplay milestone; the hook's random draws are retained.
- Validation: `cargo test --offline mapgen::` passed 64/64; `cargo test --offline --quiet` passed 758/758; `cargo build --offline --release` succeeded; targeted `rustfmt --check` and `git diff --check` passed. Seed-42 UI review passed 45 scenes and 145 checks before the RNG-only hook/retry changes; no presentation code changed in those follow-ups.

## Subsystem comparison ledger

Fill this in from current implementation code and the pinned Freeciv source,
not from old parity summaries. Add rows when a subsystem contains independent
mechanics that need separate cutover or verification.

| Subsystem | CivCraft implementation path(s) | Freeciv source / ruleset inputs | Port decision or mapping | Status | Evidence |
|---|---|---|---|---|---|
| Turn and calendar | `src/sim/turn.rs`, `src/sim/mod.rs`, `src/sim/research.rs`, `src/sim/save.rs`, `src/sim/score.rs`, `src/sim/spaceship.rs` | `server/srv_main.c::end_phase`, `begin_phase`, `check_for_game_over`; `common/calendar.c::game_next_year`; `common/game.h` victory defaults; `server/settings.c`; `civ2civ3/game.ruleset`, `effects.ruleset` | Calendar uses `Turn_Years` and `Slow_Down_Timeline` through the shared evaluator; C2C3 victory flags are explicit in the derived ruleset and gate Allied and Space Race checks. Player elimination counts any surviving unit or city. World technology history and Freeciv's deferred year-zero reset are persisted. End-phase worker automation runs before AI last activities and city processing; AI first-activity planning runs in begin phase after activity progress, movement refresh, and visibility. Barbarian uprising follows city activity, with barbarian unit planning in the next begin phase. These placements follow `srv_main.c::end_turn`, `end_phase`, `begin_phase`, and `ai_start_phase`. C2C3 disables Culture and World Peace victory. The Freeciv-only Plastics slowdown has no Civ III technology mapping and stays inactive by explicit source-catalog decision. Broader conquest/team/scenario resolution and achievement/game-over ordering are deferred to P12; detailed AI behavior remains P11. The 5000-turn end limit matches Freeciv's default, while configurability is deferred. | Complete | Calendar vectors cover all year thresholds, Space Flight/Superconductors slowdown caps, and year zero; turn tests cover diplomacy expiry, C2C3 victory flags, AI movement refresh ordering, barbarian uprising timing, and survival with units after city loss. Current `cargo test --offline --quiet`: 758/758. Freeciv engine defaults `SPACERACE|ALLIED` are explicit in `game.ruleset`; the manifest and baseline README record this derivative setting. Save header is `\x19`; earlier formats remain unsupported, matching the existing loader policy. |
| Map and topology | `src/mapgen.rs`, `src/mapgen/temperature.rs`, `src/mapgen/terrain.rs`, `src/mapgen/placement.rs`, `src/mapgen/rivers.rs`, `src/rules/freeciv.rs`, `src/world/topology.rs` | `server/generator/height_map.c::make_pseudofractal1_hmap`, `gen5rec`, `height_map_to_map`; `server/generator/mapgen.c::make_land`, `make_relief`, `make_terrains`, `make_rivers`, `make_river`, `add_resources`, `make_huts`; `server/generator/temperature_map.c::create_tmap`; `server/generator/mapgen_utils.c::pick_terrain`, `smooth_water_depth`; `server/generator/startpos.c::get_tile_value`, `create_start_positions`; `common/extras.c::rand_extra_for_tile`; `common/city.h::CITY_MAP_DEFAULT_RADIUS_SQ`; `common/map.c::rand_map_pos_filtered`; `server/gamehand.c`; `server/generator/mapgen_topology.h`; `common/terrain.h`, `common/map.h`; C2C3 `game.ruleset` and `terrain.ruleset` | The active default path ports Freeciv's recursive midpoint field, land mask, temperature, relief, terrain selection, ocean-depth assignment, rivers, resource/hut placement, and DEFAULT+FRACTAL start placement, adapted to X-wrap and bounded-Y topology. Civ III terrain/resource identities and yields are the explicit content adapter; C2C3 Swamp maps to Civ III Marsh. The derivative records engine defaults for steepness 30, wetness 50, specials 250, and huts 15. Freeciv's up-to-three map attempts use fresh gameplay-stream seeds and failed start searches retain their draws. The `map_generated` label hook's conditional draws are reproduced; its cosmetic text is not stored or rendered. The post-generation ocean-only polar boundary remains a CivCraft presentation override. | Complete | `cargo test --offline mapgen::`: 64/64; current full suite: 758/758. Source-trace tests cover height and terrain draws, river tie draws, filtered start sampling, resource/hut ordering, retry seeds, and label selection/conditional draws. `tests/fixtures/freeciv_mapgen_vectors.tsv` remains unchanged; CivCraft vectors were refreshed for intentional resource placement and resulting start-score changes. Intentional differences are adapted topology distances/latitude, Civ III resource candidates and yields instead of C2C3 resource records, and polar-ocean postprocessing. Fair-map generation and non-DEFAULT start modes remain outside P5; exact native server output parity is not claimed. Seed-42 `--ui-review` at 1600×900 passed 45 scenes and 145 checks, including generated terrain, poles, rivers, and seams; captures are in `/tmp/civcraft-p5-ui-review-20261112/`. |
| Visibility and map knowledge | `src/sim/observed_map.rs`, `src/sim/observed_map/unit_sight_bindings.rs`, `src/sim/pf_actions.rs`, `src/sim/exploration.rs`, `src/sim/diplomacy/observer.rs` | `common/vision.c`, `common/map.c`, `common/requirements.c`; `units.ruleset`, `effects.ruleset` | Unit/city radius, `vision_layer`, terrain/base bonuses, and map/city reveal use shared ruleset effects and requirements. Hidden units use C2C3 Main/Stealth/Subsurface layers and the radius-squared-2 detector cap. Pathfinding, exploration, observer checks, and map refresh share the same sight helpers. CivCraft's X-wrap/bounded-Y distance is the topology adaptation. Stable Conquests IDs resolve through a sight-only table: 33 exact records, 27 explicit aliases (18 sourced from Conquests replacement links), and 32 unresolved entries. Four direct-name aliases cover Horseman, Pikeman, Rifleman, and Mech Infantry. Unresolved entries use C2C3 baseline radius 2 and preserve Civ III stealth behavior where present; aliases do not establish full unit identity. | Complete | Tests cover all 92 runtime unit IDs, radius/layer resolution, terrain/base/technology effects, global reveals, wrapped/polar bounds, and hidden-layer detection. Of 32 unresolved identities, 26 have ambiguous candidates and six have no credible C2C3 counterpart. Guerilla remains unresolved: the Civ III kind is used both as a regular unit and for conquest-spawned Partisans, while C2C3 Partisan sight is radius-squared 6 versus the unresolved baseline 2. Spy and AWACS remain Freeciv-only identities. Current full suite: 758/758. |
| Random streams | `src/random.rs`, `src/mapgen.rs`, `src/sim/mod.rs`, `src/sim/save.rs` | `utility/rand.c`; `server/srv_main.c::init_game_seed`; `server/generator/mapgen.c::map_fractal_generate`; `server/plrhand.c::shuffle_players`; `server/gamehand.c::init_new_game` setup/start allocation | The full caller seed remains the CivCraft world ID; the Freeciv gameplay stream uses its low 32 bits and draws a separate 31-bit map seed. One map stream covers fractal, terrain, river, water-depth, resource, and hut passes. Map retries draw fresh seeds from the restored gameplay stream, and failed start-position searches retain consumed draws. Start allocation uses the gameplay stream; the `map_generated` label hook, player-slot shuffle (511 bounded calls for 512 slots), game identifier (32 draws bounded by 64), and unrestricted start-list shuffle run in source order. The start-list shuffle uses CivCraft's deterministic generated order because Freeciv's hash input order is process-address dependent; Civ III roster identity remains fixed. | Complete | Source-trace tests cover map-seed initialization/retries, bounded draw behavior, filtered start sampling, river tie draws, label hook branch draws, and post-mapgen setup state/permutation. Current `cargo test --offline --quiet`: 758/758. C2C3's `animals=14` creates Freeciv-only Storm units after `init_new_game`; these are omitted to preserve the Civ III unit catalog, so later RNG-state parity is not claimed past that omitted content. Random nation/player selection is adapted to the fixed Civ III roster; the 64-bit world seed is narrowed to 32 bits. |
| City output and citizens | `src/sim/economy.rs`, `src/sim/cities.rs`, `src/sim/citizens.rs` | `common/city.c::city_turns_to_build`, `common/citizens.c`; `server/cityturn.c::update_city_activities`, `update_city_activity`; `cities.ruleset`, `effects.ruleset` | Current: city forecast and manager are Freeciv-derived; output/effects remain partly hardcoded. Foreign nationality, assimilation, and resistance are Civ III-specific and need Freeciv citizen/migration behavior. | Not started |  |
| Production and upkeep | `src/sim/cities/production.rs`, `src/sim/upkeep.rs`, `src/sim/buildings.rs` | `server/cityturn.c::update_city_activity`, `city_build_stuff`; `server/unittools.c`; `buildings.ruleset`, `units.ruleset`, `effects.ruleset` | Current: phase and production behavior partly track Freeciv, but building catalog is still Freeciv-derived (its source comment pins `eb8c7033`, not the selected revision) and upkeep/effects include Civ III rules. Map Civ III content into the Freeciv model. | Not started |  |
| Research and government | `src/sim/research.rs`, `src/sim/government.rs`, `src/rules/research.rs` | `common/research.c::research_allowed`, `research_update`; `server/plrhand.c`; `governments.ruleset`, `techs.ruleset`, `effects.ruleset` | Current: Freeciv tech catalog/progression with Civ III additions and an era-completion gate; government roster/support are Civ III-specific. Replace catalog authority with Civ III tech tree and Freeciv mechanics. | Not started |  |
| Movement and actions | `src/sim/movement.rs`, `src/sim/pf_actions.rs`, `src/sim/commands.rs`, `src/sim/normal_paths.rs` | `common/movement.c::unit_move_to_tile_test`, `unit_move`; `common/actions.c`; `common/aicore/pf_tools.c`; `actions.ruleset`, `units.ruleset`, `terrain.ruleset` | Current: costs/actions are substantially Freeciv-derived, but powered by static Rust data and Civ III topology; ZOC movement-blocking stubs return false. Port after topology and ruleset data. | Not started |  |
| Combat and military | `src/sim/movement/combat.rs`, `src/sim/aviation.rs`, `src/sim/nuclear.rs` | `common/combat.c::unit_win_chance`, `get_defender`, `get_modified_firepower`; `server/unittools.c`; `actions.ruleset`, `units.ruleset`, combat settings | Current: Freeciv battle odds/defender selection with Civ III HP, data, and special effects; stack losses differ under Freeciv default `killstack`. Keep Civ III unit records, use ruleset mechanics. | Not started |  |
| Diplomacy, espionage, and trade | `src/sim/diplomacy/`, `src/sim/tech_trade.rs` | `server/diplhand.c`, `server/diplomats.c`; `common/diptreaty.c`; `ai/default/daidiplomacy.c`; `actions.ruleset`, `effects.ruleset` | Current: bespoke pact/concession state with selected Freeciv AI formulas. Port treaty clauses and actions; map Civ III espionage without substituting Freeciv Diplomat/Spy unit content. | Not started |  |
| Culture, borders, and migration | `src/sim/culture.rs`, `src/sim/borders.rs`, `src/sim/citizens.rs` | `common/culture.c::city_culture`, `city_history_gain`, `player_culture`; `common/borders.c::tile_border_source_radius_sq`, `tile_border_source_strength`; `server/cityturn.c::city_migration_score`, `do_city_migration`; `server/maphand.c::map_update_border`, `map_claim_border`; `game.ruleset`, `effects.ruleset`, inherited default Lua | Current: Civ III building culture, culture-ranked borders, capture history, and flip candidate. Replace with Freeciv culture scores, radius/strength borders, citizen migration, and configured victory. | Not started |  |
| AI | `src/sim/ai.rs`, `src/sim/ai_economy.rs`, `src/sim/workers.rs`, `src/sim/settlement.rs`, `src/sim/ai/` | `ai/default/daihand.c::dai_manage_taxes`, `daicity.c::dai_manage_cities`, `daiunit.c::dai_manage_units`, `server/advisors/autoworkers.c`, `common/aicore/` | Current: tax, wants, and diplomacy include Freeciv ports; assault fleets, settlement reservations, hunter/guard behavior are CivCraft-specific. Port after mechanics and rule data stabilize. | Not started |  |
| Climate and disasters | `src/sim/climate.rs`, `src/sim/disasters.rs` | `server/srv_main.c::update_environmental_upset`, `check_disasters`; `server/cityturn.c::can_disaster_happen`, `apply_disaster`; `server/maphand.c::climate_change`; `common/disaster.c`; `game.ruleset`, `effects.ruleset`, `terrain.ruleset` | Current: climate is a close Freeciv port needing draw-order/edge parity; disasters reproduce outcomes with hardcoded Rust branches rather than generic ruleset effects. | Not started |  |
| Villages and spaceship | `src/sim/villages.rs`, `src/sim/spaceship.rs` | inherited `data/default/default.lua::_deflua_hut_enter_callback`; `common/spaceship.c::next_spaceship_component`; `server/spacerace.c::do_spaceship_place`, `spaceship_arrival`, `spaceship_arrived`; spaceship/game settings | Current: hut outcome weights and spaceship attachment order are Freeciv-derived, but adapted unit rewards and launch/arrival paths need comparison. | Not started |  |
| Score and victory | `src/sim/score.rs`, `src/sim/achievements.rs`, `src/sim/turn.rs`, `src/sim/culture.rs` | `server/score.c::calc_civ_score`, `get_civ_score`; `server/srv_main.c` victory checks; `common/victory.c`; `game.ruleset`, inherited default Lua | Current: score weights partly Freeciv-derived; achievement culture, cultural victory check, and game-over semantics are custom. Port configured Freeciv victory and scoring behavior. | Not started |  |

## Validation standard

- Pin the Freeciv source and ruleset revision for every comparison fixture.
- For formulas and isolated transitions, compare exact outputs with a small
  reference harness or source-derived vectors.
- For randomized mechanics, compare distributions and random draw/order
  behavior where the port intends deterministic parity; record unavoidable
  differences explicitly.
- For turn-level behavior, compare resulting state across deterministic
  scenarios, including boundary cases and interactions with the preserved
  Civ III catalogs.
- Keep gameplay validation separate from UI/presentation validation. Client
  changes should consume simulation outputs through existing client views.
- Do not mark a mechanic complete solely because it has a passing test. Its
  expected behavior must trace to the pinned Freeciv implementation and
  `civ2civ3` configuration.

## Completion criteria

The port is complete for the declared campaign scope when:

1. Every in-scope gameplay subsystem is marked **Complete** in the work plan
   and comparison ledger, with source paths and evidence.
2. CivCraft's editable ruleset is the sole source of gameplay settings,
   requirements, effects, action availability, and tunable ruleset values.
3. Civ III unit, building, technology, and art content remain the selected
   campaign content and are loaded through explicit mappings.
4. No migrated mechanic depends on hidden CivCraft constants or duplicated
   calculations outside the shared ruleset model.
5. Remaining differences are explicitly listed as out of scope or intentional
   extensions, with their behavior and rationale recorded.
6. The current client architecture and saves have an explicit compatibility
   outcome for the completed migration.

## Scope assumption

This plan covers the playable single-player campaign in CivCraft. Freeciv's
network protocol, multiplayer server administration, graphical clients, ruleset
editor, and general mod-pack distribution are excluded unless added as a
separate approved scope item. The ruleset data model is still designed to allow
editing and creating CivCraft rulesets.
