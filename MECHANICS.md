# Mechanics source sheet

This sheet is the contract for the values used by the Rust build. Paths beginning with reference/freeciv are relative to the supplied target. Deep-mode decompilation of the original arm64 server establishes the native functions, addresses, control flow, and arithmetic; the readable Civ2Civ3 ruleset supplies the runtime data those functions load, and the matching C source resolves types that the register-level lifter cannot recover reliably.

## Native verification

The verified binary is `freeciv-server` with SHA-256 `2388ee9802fd555082c9c4979685c70e276d957e025484eb34a744e71653b2ff`. Addresses are specific to that binary.

The native trace began with complete string searches and then followed each executable reference. The important entry traces were:

| Search class | String address | Executable reference | What it established |
| --- | --- | --- | --- |
| Unit roles | `?unitflag:Workers` at `0x100468349` | `_unit_type_flag_id_name` at `0x10003af68` | Worker behavior is selected by a unit flag; concrete unit values are loaded from the ruleset at runtime. |
| Unit roles | `?unitflag:Explorer` at `0x1004707f9` and `?unitflag:Settlers` at `0x100470a83` | `_unit_role_id_name` at `0x100067ad0` | Explorer and city-founder behavior use named roles. |
| Settings | `topology` at `0x100474592` | `_setting_bitwise_secfile_str` at `0x10013a730` | The helper's `WrapX`/`WrapY` branch is a legacy combined-setting compatibility path; it must not be read as the current runtime layout. |
| Settings | `WrapX|WrapY` at `0x10047459b`, `ISO|HEX` at `0x1004745b3`, and `wrap` at `0x1004745c3` | `_compat_load_030200` at `0x100073560` | Old saves are migrated to separate settings: topology is `ISO|HEX`, while wrapping is independently `WRAPX|WRAPY`. The build therefore keeps topology and wrapping separate. |
| Settings | `aifill` at `0x10048b549` | `_aifill` at `0x10014cb50` | The setting creates or removes AI players to reach the requested player count. |
| Settings | `tilesperplayer` at `0x10048b569` | `_send_to_metaserver` at `0x1001154f0`, then map topology callers | The setting is reported as the land-tile target and consumed by topology initialization. |
| Settings | `sciencebox` at `0x100494c5d` | `_luaopen_game` at `0x1002a9174`, then research callers | The global percentage is a final multiplier in the research-cost formula. |
| Message | map-size messages at `0x100466eb5` and `0x100467078` | `_generator_init_topology` at `0x10003158c` and `_set_sizes` at `0x1000322f8` | The first reports the selected size mode; the second confirms the final native dimensions and requested tile count. |
| Message | famine message at `0x100480ecf` | `_city_populate` reference at `0x1000ec770` | Leads to growth, shrink, and size-one city destruction. |
| Message | completion message at `0x100480b6e` | `_city_build_building` reference at `0x1000ed8f0` | Leads to the separate building and unit production paths. |
| Message | unit-completion message at `0x100480d7f` | `_city_build_unit` reference at `0x1000ee1d0` | Confirms that unit completion separately applies shield and population costs before creating the unit. |
| Message | combat-power message at `0x1004a7559` | `_unit_versus_unit` reference at `0x10018f228` | Leads to attack, defense, firepower, and repeated combat rounds. |

| Area | Native function | Address | Rule verified in this build |
| --- | --- | --- | --- |
| Topology | `_generator_init_topology` | `0x100031444` | Player-count, land-percentage, and tiles-per-player values determine requested map size. |
| Topology | `_get_ratios` | `0x100031e9c` | A map wrapped on both axes uses the 1:1 torus ratio. |
| Topology | `_set_sizes` | `0x100031f5c` | Uses `trunc(sqrt(requested / (xratio × yratio × iso × even²)) + 0.49)`, with `even = 2` and `iso = 2`, then derives both dimensions. |
| Generation | `_map_fractal_generate` | `0x10001fef4` | The standard map path uses fractal generation. |
| Generation | `_make_land` | `0x10002437c` | Land generation is driven by the configured land percentage. |
| Distance | `_map_distance_vector` | `0x100395e70` | Wrapped native coordinates are converted before distance is measured. |
| Distance | `_base_map_distance_vector` | `0x10039859c` | Topology-aware map vectors choose the shortest wrapped displacement. |
| Distance | `_map_vector_to_real_distance` | `0x100395c10` | Isometric-hex distance is `abs(dx) + abs(dy)` across the forbidden diagonal and `max(abs(dx), abs(dy))` otherwise. |
| Distance | `_real_map_distance` | `0x100395e30` | City spacing and AI range use the topology-aware distance. |
| Distance | `_is_tiles_adjacent` | `0x100398030` | Only one-step topology neighbors are adjacent. |
| Movement | `_tile_move_cost_ptrs` | `0x100396d8c` | Destination terrain, roads, and unit terrain-ignore rules determine entry cost. |
| Cities | `_citymindist_prevents_city_on_tile` | `0x100374f20` | Founding is rejected inside the configured minimum city distance. |
| Cities | `_city_can_be_built_here` | `0x1003751a8` | Full city founding checks combine terrain and spacing rules. |
| Cities | `_city_can_be_built_tile_only` | `0x100375420` | Ocean and other no-city terrain are rejected before founding. |
| Growth | `_city_granary_size` | `0x1003767e8` | Growth boxes use the configured initial sizes and foodbox percentage. |
| Growth | `_city_populate` | `0x1000ec2d0` | Food surplus grows a city; negative food triggers famine handling. |
| Growth | `_city_reduce_size` | `0x1000e2464` | Famine removes population and destroys a size-one city. |
| Growth | `_city_increase_size` | `0x1000e3064` | Successful growth consumes the current food box and increases population. |
| Production | `_city_build_stuff` | `0x1000ec14c` | Finished production dispatches to the matching unit or building path. |
| Production | `_city_build_unit` | `0x1000edbe8` | Unit completion applies shield and population costs. |
| Production | `_city_build_building` | `0x1000ed3f4` | Building production is a separate class, outside this opening unit-only slice. |
| Production | `_city_change_production_penalty` | `0x100376488` | Changing from one unit to another keeps accumulated shields because the production class is unchanged. |
| Research | `_research_total_bulbs_required` | `0x1003e236c` | Applies player `Tech_Cost_Pct`, leakage, AI difficulty, and `sciencebox` to the selected advance's precalculated cost. |
| Research | `_techs_precalc_data` | `0x1003ecc00` | Advance costs are precalculated from the configured cost style and prerequisites. |
| Combat | `_unit_versus_unit` | `0x10018f110` | Combat repeats weighted attack-versus-defense rounds until one unit reaches zero HP. |
| Combat | `_get_modified_firepower` | `0x10037c498` | Each winning round removes the opposing unit's modified firepower. |
| Combat | `_get_total_attack_power` | `0x10037c964` | Total attack power layers unit and effect modifiers. |
| Combat | `_get_attack_power` | `0x10037cdb4` | Reads the unit's type, veteran level, and remaining movement before delegating to the base formula. |
| Combat | `_base_get_attack_power` | `0x10037cdec` | Tired attack scales power by remaining movement fragments. |
| Combat | `_is_tired_attack` | `0x10037cd58` | The tired-attack rule activates below one full move. |
| Combat | `_get_total_defense_power` | `0x10037c088` | Terrain, city, and fortification effects modify defense power. |
| Combat | `_base_get_defense_power` | `0x10037cf24` | Base defense is unit defense times the power factor and veteran percentage. |
| Combat | `_get_defense_power` | `0x10037d664` | Base defense is multiplied by the terrain stage first, with integer truncation. |
| Combat | `_defense_multiplication` | `0x10037d348` | City `Defend_Bonus` and `Fortify_Defense_Bonus` are later, separate multiplication stages. |
| Combat | `_win_chance` | `0x10037c2f8` | Sums the binomial probability that the attacker lands its required hits before taking its own lethal number of hits. |
| Combat | `_unit_win_chance` | `0x10037c8d4` | Supplies current attack, defense, HP, and modified firepower to the win-chance formula. |
| Combat | `_get_defense_rating` | `0x10037daa4` | Rates defense as defense power times rounds survived times defender firepower. |
| Combat | `_get_defender` | `0x10037d7f8` | Chooses the defender by full combat odds, then lower build cost, then higher defense rating. |
| Recovery | `_hp_gain_coord` | `0x1003fee50` | Takes the greater of percentage regeneration and minimum HP, then adds the separately truncated `(max_hp / 10) × HP_Regen_2 / 10` term. |
| Recovery | `_unit_gain_hitpoints` | `0x1003fef90` | A unit that moved does not receive normal turn regeneration. |
| Recovery | `_unit_restore_hitpoints` | `0x100190388` | End-of-turn healing is capped at the unit type's maximum HP. |
| AI | `_dai_military_attack` | `0x100458244` | Military units evaluate and move toward attack targets. |
| AI | `_manage_auto_explorer` | `0x100013bcc` | Automated explorers seek reachable unexplored tiles. |
| AI | `_auto_worker_findwork` | `0x100018c20` | Automated workers search for legal terrain improvements. |
| AI | `_dai_do_build_city` | `0x1004473c4` | AI settlers validate and found at candidate city sites. |
| AI | `_advisor_choose_build` | `0x10000d468` | City production is selected through the advisor path. |

Every function listed above, plus the seven string-trace helpers, was requested separately from `native_function_pseudocode`, and all 53 results were awaited. The Ghidra bridge returned no result for them, so Craft Anything's built-in lifter supplied register-level pseudo-C instead. That output has approximate types and conditions, unresolved instructions, and some unstructured branches; function identities, addresses, calls, constants, and strings were used as native confirmation, while exact types and runtime-loaded values were cross-checked against the supplied C source and Civ2Civ3 rulesets.

## Match

| Value used | Number or rule | Source |
| --- | --- | --- |
| Ruleset | Civ2Civ3 | reference/freeciv/common/game.h, GAME_DEFAULT_RULESETDIR |
| Players | 6 total: one person and five computer rivals | reference/freeciv/data/civ2civ3/game.ruleset, aifill = 6 |
| Source map target | 100 land tiles per player | reference/freeciv/data/civ2civ3/game.ruleset, tilesperplayer = 100 |
| Requested map area | 2,000 tiles | `_generator_init_topology` at `0x100031444`: `6 players × 100 land tiles × 100 / 30 land-percent` |
| Build map | 32 by 64 wrapped native tiles, 2,048 total | `_get_ratios` at `0x100031e9c` returns 1:1 for two-axis wrap. `_set_sizes` at `0x100031f5c` computes `scale = trunc(sqrt(2000 / (1 × 1 × 2 × 2²)) + 0.49) = 16`, then `width = 1 × 16 × 2` and `height = 1 × 16 × 2 × 2`. |
| Land target | 30% | CivCraft uses the Civ2Civ3 land percentage when placing the shoreline. |
| Generator | Local pseudofractal elevation, climate, relief, biome patches, rivers, water smoothing, lakes, resources, villages, and start placement | Reference `server/generator/mapgen.c`, `height_map.c`, and `startpos.c`. The Rust implementation is checked against Freeciv revision eb8c7033 for the default six-player Civ2Civ3 setup, using the same value for mapseed and gameseed. Regression checks cover terrain, elevation, extras, and start allocation. |
| Wrap | CLI `--wrap x` (default) or `--wrap xy`; per-campaign cylinder or torus | reference/freeciv/data/civ2civ3/game.ruleset, WRAPX and WRAPY |
| Starting year | 4000 BCE | reference/freeciv/data/civ2civ3/game.ruleset, calendar.start_year = -4000 |
| Early calendar | 50 years per turn before 1000 BCE | reference/freeciv/data/civ2civ3/effects.ruleset, effect_calendar_base |
| Later calendar | 25, 20, 10, 5, 2, then 1 year per turn at -1000, 0, 1000, 1500, 1750, 1900 | reference/freeciv/data/civ2civ3/effects.ruleset, effect_calendar_1 through effect_calendar_6 |
| Starting gold | 50 | reference/freeciv/common/game.h, GAME_DEFAULT_GOLD |
| Starting units | one Settlers, Worker, Explorer, and Diplomat | reference/freeciv/data/civ2civ3/game.ruleset, startunits = cwsx; server/settings.c explains the role letters |
| Starting advances | 1 free root advance | reference/freeciv/data/civ2civ3/game.ruleset, techlevel = 1 |
| City minimum distance | 3 tiles | reference/freeciv/data/civ2civ3/game.ruleset, citymindist = 3 |
| Fog and borders | enabled, including fogged borders | reference/freeciv/data/civ2civ3/game.ruleset |
| Turn timer | none | reference/freeciv/common/game.h, GAME_DEFAULT_TIMEOUT = 0 |
| Turn limit | 5000 | reference/freeciv/common/game.h, GAME_DEFAULT_END_TURN |
| Focused victory | conquest when no rival city or city founder remains | Freeciv conquest remains possible; focused-build scope decision |

## Movement and view

| Value used | Number or rule | Source |
| --- | --- | --- |
| Movement fragments | 6 per movement point | reference/freeciv/data/civ2civ3/terrain.ruleset, move_fragments = 6 |
| Explorer rough-terrain cost | 2 fragments, or one third of a point | terrain.ruleset igter_cost = 2 and units.ruleset Explorer IgTer flag |
| Road cost | 2 fragments | reference/freeciv/data/civ2civ3/terrain.ruleset, road_road.move_cost |
| Railroad cost | 1 fragment | reference/freeciv/data/civ2civ3/terrain.ruleset, road_railroad.move_cost; recorded though not in this era slice |
| Maglev cost | 0 fragments | reference/freeciv/data/civ2civ3/terrain.ruleset, road_maglev.move_cost; recorded though not in this era slice |
| Vision radius squared | 2 for normal opening units, 6 for Explorer | reference/freeciv/data/civ2civ3/units.ruleset |
| Isometric source tile | 96 by 48 pixels | reference/freeciv/data/amplio2.tilespec |
| Starting zoom | 0.82 | Estimated to keep the opening units and nearby terrain readable in a 1280 by 720 window |
| Zoom range | 0.55 to 1.35 | Estimated usability range; the source clients expose zoom but do not share one fixed range |
| Camera field of view | not applicable; orthographic map | Turn-based isometric client |

Terrain entry cost is paid when a land unit enters the destination. As in Freeciv, a unit with movement remaining may enter a more expensive rough tile and finish at zero. Water blocks the opening land units. `_base_map_distance_vector` at `0x10039859c` first wraps each native delta into `[-dimension / 2, dimension / 2)`, then converts the wrapped endpoint back to map coordinates. `_map_vector_to_real_distance` at `0x100395c10` returns `abs(dx) + abs(dy)` when an iso-hex vector points northeast or southwest (the forbidden diagonals), and `max(abs(dx), abs(dy))` otherwise. Movement, visibility, city range, AI targets, and pathfinding share that vector, so crossing either edge remains one legal hex step.

## Terrain

Base yields are food, shields, trade.

| Terrain | Move points | Defense | Base yields | Source |
| --- | ---: | ---: | --- | --- |
| Ocean | blocked for land units | 0% | 1, 0, 2 | civ2civ3/terrain.ruleset terrain_ocean |
| Desert | 1 | 0% | 0, 1, 0 | terrain_desert |
| Forest | 2 | +25% | 1, 2, 0 | terrain_forest |
| Grassland | 1 | 0% | 2, 0, 0 | terrain_grassland |
| Hills | 2 | +50% | 1, 1, 0 | terrain_hills |
| Mountains | 3 | +100% | 0, 1, 0 | terrain_mountains |
| Plains | 1 | 0% | 1, 1, 0 | terrain_plains |
| Swamp | 2 | +25% | 1, 0, 0 | terrain_swamp |
| Tundra | 1 | 0% | 1, 0, 0 | terrain_tundra |

Road work takes 2 turns on one-cost land, 4 on two-cost land, and 6 on mountains. Irrigation normally takes 5 turns, or 10 on hills. Mines take 5 turns on desert or tundra and 10 on hills or mountains. These values come from each matching terrain section in civ2civ3/terrain.ruleset. The build applies the matching listed food or shield increment and a one-trade road bonus only on terrain whose ruleset percentage permits it. Basic irrigation requires an edge-adjacent ocean or irrigated tile; irrigation and mines conflict, so finishing either removes the other. A city center receives its terrain's irrigation bonus automatically but does not itself become an irrigation source.

## Opening units

Movement is in whole movement points before conversion to 6 fragments.

| Unit | Cost | Population cost | Attack | Defense | HP | Firepower | Move | Source |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Settlers | 30 | 2 | 0 | 1 | 20 | 1 | 1 | civ2civ3/units.ruleset unit_settlers |
| Workers | 20 | 0 | 0 | 1 | 10 | 1 | 1 | unit_worker |
| Diplomat | 30 | 0 | 0 | 0 | 10 | 1 | 2 | unit_diplomat |
| Explorer | 30 | 0 | 0 | 1 | 10 | 1 | 1 | unit_explorer |
| Warriors | 10 | 0 | 1 | 1 | 10 | 1 | 1 | unit_warriors |
| Phalanx | 20 | 0 | 1 | 2 | 10 | 1 | 1 | unit_phalanx |
| Archers | 20 | 0 | 3 | 1 | 10 | 1 | 1 | unit_archers |
| Legion | 30 | 0 | 4 | 2 | 10 | 1 | 1 | unit_legion |
| Horsemen | 20 | 0 | 2 | 1 | 10 | 1 | 2 | unit_horsemen |

Phalanx requires Bronze Working, Archers require Warrior Code, Legion requires Iron Working, and Horsemen require Horseback Riding. Settlers and Warriors have no advance requirement in the source ruleset.

## Combat and recovery

- Each combat round gives the attacker a hit when a random value in attack power plus defense power is at least defense power; otherwise the defender hits. With no positive `Combat_Rounds` effect in this opening ruleset, rounds have no fixed cap and repeat until one unit reaches zero HP. Source: reference/freeciv/server/unittools.c, `unit_versus_unit` at `0x10018f110`.
- Each hit removes the attacker's or defender's firepower. Every opening unit above has firepower 1. Source: civ2civ3/units.ruleset.
- `_win_chance` at `0x10037c2f8` uses the same repeated-round process analytically. Let `p = attack / (attack + defense)`, `A = ceil(attacker HP / defender firepower)`, and `D = ceil(defender HP / attacker firepower)`. Then `P(attacker wins) = p^D × sum(r = 0..A-1, C(D - 1 + r, r) × (1 - p)^r)`. If both powers are zero, the per-round probability is one half.
- When multiple units occupy the target, `_get_defender` at `0x10037d7f8` maximizes `floor(100000 × (1 - P(attacker wins)))`. Equal values prefer the lower build cost, then the larger `_get_defense_rating` value `defense power × ceil(defender HP / attacker firepower) × defender firepower`. The Rust build now uses this rule instead of selecting only by raw defense.
- Defense is calculated in ordered integer stages, not by adding every percentage into one bonus: `base defense × (100 + terrain) / 100`, then `× (100 + Defend_Bonus) / 100`, then `× (100 + Fortify_Defense_Bonus) / 100`. Source: `_get_defense_power` at `0x10037d664` and `_defense_multiplication` at `0x10037d348`.
- A city gives ground defenders a +50% `Defend_Bonus`; size 9 adds another +50% within that same stage. A can-fortify ground unit in a city also receives a +50% `Fortify_Defense_Bonus` even when it has not explicitly fortified. Explicit fortification supplies that same stage outside a city and becomes active on the following turn. Thus a regular 10-power defender on grassland has power 10 in the field, 15 fortified in the field, and 22 in a size-one city (`10 × 150% × 150%`, with integer truncation). Source: `effect_city_defense_0`, `effect_city_defense_1`, `effect_city_fortified`, and `effect_fortified` in Civ2Civ3 effects.ruleset.
- Tired attack is enabled: below one full movement point, attack power is multiplied by remaining fragments divided by 6. Source: civ2civ3/game.ruleset combat_rules and common/combat.c base_get_attack_power.
- A successful attacker occupies the target tile with 100% chance. Source: civ2civ3/game.ruleset occupychance = 100.
- `_hp_gain_coord` at `0x1003fee50` calculates `max(floor(max HP × HP_Regen / 100), floor(max HP × Min_HP_Pct / 100)) + floor(floor(max HP / 10) × HP_Regen_2 / 10)`. The original C spells the last numerator as `ceil(base / 10)`, but `base / 10` is integer division before `ceil`; the arm64 pseudo-C confirms an integer `sdiv` by 10 first. Opening ground units have 0% `HP_Regen`, a universal 10-point `HP_Regen_2`, a city-center `Min_HP_Pct` of 33, and another 10 points of `HP_Regen_2` while fortified. A damaged 10-HP unit therefore gains 1 HP in the field, 4 HP in a city, or 5 HP while fortified in a city; a 20-HP Settlers unit gains 2 HP in the field or 8 HP in a city. `_unit_gain_hitpoints` at `0x1003fef90` suppresses normal regeneration after movement, and `_unit_restore_hitpoints` at `0x100190388` caps healing at maximum HP.
- There are no magazines, reloads, spread, recoil, projectile range, armour, or real-time respawns in this turn-based strategy game.

## Cities and research

| Value used | Number or rule | Source |
| --- | --- | --- |
| Citizen food use | 2 food per citizen per turn | civ2civ3/game.ruleset food_cost = 2 |
| Growth box sizes 1 through 8 | 20, 20, 20, 20, 20, 30, 30, 40 food | civ2civ3/game.ruleset granary_food_ini |
| Growth after size 8 | 40 food | granary_food_inc = 0 |
| Settler population cost | 2 citizens when produced | civ2civ3/units.ruleset |
| Base research style | linear | civ2civ3/game.ruleset tech_cost_style |
| Pre-effect base and minimum research | 10 bulbs | civ2civ3/game.ruleset base_tech_cost and min_tech_cost |
| Player technology-cost effect | 300% | civ2civ3/effects.ruleset effect_tech_cost_base |
| Final `sciencebox` multiplier | 100% | common/game.h GAME_DEFAULT_SCIENCEBOX and server/settings.c |
| Default tax rates | 0% tax, 100% science, 0% luxury | reference/freeciv/common/player.h |
| Root advance cost | 30 bulbs | `max(10, 10 × 1 recursive requirement) × 300% × 100%` |
| Iron Working cost | 90 bulbs | `10 × 3 recursive requirements × 300% × 100%`; Bronze Working and Warrior Code are its prerequisites |

`_city_granary_size` at `0x1003767e8` uses the entry at `granary_food_ini[size - 1]` while one exists; afterward it uses `last initial value + granary_food_inc × (size - initial-count)`. It then applies `foodbox / 100` and clamps the result to at least 1. With `[20, 20, 20, 20, 20, 30, 30, 40]`, increment 0, and foodbox 100%, the build's boxes are exactly the values above. `_city_populate` at `0x1000ec2d0` consumes that box on growth; `_city_reduce_size` at `0x1000e2464` makes famine remove one citizen and permits a size-one city to be destroyed.

The native research formula first uses `_techs_precalc_data` at `0x1003ecc00` to calculate `max(minimum cost, base cost × recursive requirement count)`. `_research_total_bulbs_required` at `0x1003e236c` then multiplies that value by the player's `Tech_Cost_Pct`, applies any technology leakage and AI science factor, and finally multiplies by `sciencebox`. The opening clone has no embassies, so the configured embassy leakage does not reduce these costs; normal AI has a 100% science factor.

Civ2Civ3 adds one shield to city-center tile output and configures all three city-center minimums as zero. The additive effect is applied before tile penalties and contamination; it is not a minimum-one substitute. Workers use the city radius, with specialists and tile assignments tracked separately. `_city_build_unit` at `0x1000edbe8` completes a unit only after the city has its shield cost and, for population-cost units, `city size > population cost`; completion subtracts both costs. `_city_change_production_penalty` at `0x100376488` keeps all accumulated shields when switching from one unit target to another because both targets share the unit production class. Buildings and wonders use a separate completion path corresponding to `_city_build_building` at `0x1000ed3f4`, with their ruleset requirements checked before production is offered.

Factory and Mfg. Plant each add 25 percent shield output. An effective power source adds another 25 percent for each industrial building; multiple power sources do not stack. Hoover Dam's shield effect applies in its own city, as specified by its City-range requirement. Output is multiplied and rounded before shield waste is calculated, following `common/city.c:set_city_production` and `data/civ2civ3/effects.ruleset`.

## Controls

Arrow and numeric direction movement, B Build City, G Go to Tile, X Auto Explore, A Auto Worker, S Sentry, F Fortify, M Mine, I Irrigate, R Road, W Wait/Space Done Moving, and Shift+Enter End Turn follow reference/freeciv/client/gui-qt/shortcuts.cpp, client/gui-gtk-4.0/gui_main.c, and client/gui-sdl3/menu.c. Mouse selection, right-click goto, middle-drag pan, minimap recentering, and wheel zoom follow the role of the source clients; exact mouse gestures differ among those clients.

## Computer players

The single-player AI uses the supplied Freeciv default AI and Civ2Civ3 rules as its reference. The principal Rust modules are `sim/advisors.rs` (production and spending), `sim/ai.rs` (unit dispatch), `sim/aviation.rs` (aircraft), `sim/workers.rs`, `sim/exploration.rs`, `sim/research.rs`, `sim/settlement.rs`, and `sim/diplomacy.rs`.

Production candidates compete through retained wants rather than a fixed category order. Defense emergencies, domestic improvements, settlers and ferries, merchants, diplomatic agents, hunters, aircraft, and spaceship construction participate. Routine treasury spending uses those production priorities while preserving reserves. Domestic evaluation includes weighted city output, pollution, happiness, construction cost, stock switching penalties, obsolescence, training, recovery, and airlift effects. Space production distinguishes Apollo from ship parts and checks empire leadership or an existing space race. A designated wonder city retains productive legal projects or is chosen using helper reachability, continent size, shield surplus, coastal status, and threats. Other domestic construction and expansion receive reduced priority there. Great and Small Wonder candidates project effects across owned cities in a scratch simulation; helper requests account for travel, existing caravans, and contributions already queued. Small Wonders cannot receive caravan shields under the Civ2Civ3 action requirements. Sources: `ai/default/daicity.c`, `daieffects.c`, `daiair.c`, and `server/score.c`.

War plans persist directional preparation and post-declaration stubbornness. The AI respects peace while preparing, handles senate obstruction, reacts to allies and casus belli, and warns rival spaceship builders before launch-triggered escalation. The supported fixed aggression default is 50; the six historical civilizations retain common AI trait defaults rather than loading Freeciv nation traits. Trust evolves with peace, provocation, gifts, trespass, wonder effects, and war desirability. Trespass uses current observer visibility, including stealth detection, cargo concealment, city garrisons, and shared vision. Supported diplomatic actions apply their incident-specific immediate love penalties, and betrayal uses recorded treaty history. Negotiation timers persist; target and preparation exclusions follow the source cadence. Pure pact proposals precede trusted allied technology/gold/vision sharing. Treaty valuations allow gold to compensate for payable greed while preserving hard alliance restrictions and the AI's minimum city holdings. Sources: `ai/default/daidiplomacy.c`, `ai/aitraits.c`, and `common/aicore/aisupport.c`.

Aircraft compare legal enemy targets using combat profit, stack value, suicide cost, balanced production cost, and amortization. Fighters can attack surface targets; aircraft compare bases for repair time and immediate defense needs, return for fuel, and nuclear weapons and independent cruise missiles use the same economic target advisor. Distant ground attacks and production plans forecast buildable trained counter-defenders, preserving stronger existing veterans; forecasts do not change live units. Arrival times use the reference movement rounding. Ground units retain explicit city invasion destinations; aircraft use those missions and carried occupation units to assess city support. Real ground missions require war. Diplomatic and naval route estimates use movement costs. Sources: `ai/default/daiair.c`, `daitools.c`, `daiunit.c`, `daidiplomat.c`, and `daihunter.c`.

AI research propagates technology wants to unknown prerequisites and scores goals using `ai/default/daitech.c`. The advisor can prefer a distant advance, but subject changes reject it until its immediate prerequisites are known. Completed research follows its existing goal, or reservoir-samples legal alternatives when that goal is finished. Research management accounts for invested bulbs and the default 100 percent subject-change penalty. Discovery overflow is exempt during its phase; acquiring an unrelated technology does not exempt existing research. Sources: `server/techtools.c` and `server/srv_main.c`.

Merchant valuation accounts for owned back routes lost during replacement, arrival discounts, marketplace opportunity cost, and wonder rush-cost reductions. The supplied Civ2Civ3 settings disable both national trade types; the reference regular caravan production advisor consequently finds no destination through its guarded foreign-search branch. Wonder helpers have a separate advisor, and existing merchants can still trade. New merchant tasks draw allied-only or nonwar scope; valid destinations are retained. Boat waiting uses preserved birth turns and source age thresholds. Merchants can rendezvous with ferries on ordinary coastal land or in ports. Exhausted merchants do not replan or consume scope draws. Sources: `common/aicore/caravan.c`, `ai/default/daidomestic.c`, and `data/civ2civ3/game.ruleset`.

Settlement values project the advisor's government goal, including locked future governments, without changing live government or rates. They use shared tile output, city crowding, per-tile growth rounding, waste, floating-point defense/coastal bonuses, and movement costs clipped to the current turn. Multiple founders reserve destinations and initial worker tiles; retained assignments coordinate land and ferry plans. Founding requires positive movement. Sources: `ai/default/daisettler.c` and `common/aicore/citymap.c`.

Terrain workers evaluate improvements around owned cities using `server/advisors/autoworkers.c`'s tile weights, worked/unworked comparison, travel and activity discounts, road connectivity, dependency improvements, and environmental urgency. Founders compare new city missions against useful work; founders and other workers share a transient per-phase reservation map with arrival-time displacement. Existing safe work continues, unsafe work is reconsidered, and idle workers seek shelter. The default AI produces no manual city work requests; the planner accepts requests from a caller, but no manual/texAI request interface is exposed.

Exploration uses `server/advisors/autoexplorer.c`'s known-map target search, coastline and hut scoring, movement-cost discount, and separate treaty-safe, risk-aware route. The source's actual unknown-center estimate, heap ties, and 16-bit cached risk conversion are preserved. Human automation clears when exploration finishes or pauses without progress. Barbarian leaders use a separate source manager: they wait in protective stacks, seek reachable friendly units, hand off ferries, and flee using reverse threat and escape-distance maps.

Save format V13 retains war plans, space-warning timers, research exemptions, invasion destinations, merchant missions and birth turns, founder reservations, negotiation memory, ferry bookings, government evaluation caches, finite diplomatic contact deadlines, environmental heating/cooling readings, typed bodyguard links, celebration counters, recorded movement destinations, independent player-unit, tile-arrival and player-city orders, the full native random state, and append-ordered cargo lists. V1 through V12 migrate with defaults for state they did not store; only V1–V8 grant previously met surviving factions one bounded contact grace period. V9–V12 preserve their existing contact deadlines. V1–V12 retain the scalar random generator used by those campaigns, including after resaving as V13. V1–V11 infer entity creation order; their historical arrivals and ownership-transfer order cannot be recovered. V1–V12 infer cargo order from stored units because boarding history was not saved. Both earlier development V7 strategy layouts are recognized. The campaign playtest checks retained state, native task resets, identical continuation from two loads of the same save, and legacy migrations. Native loading clears destinations for units without queued orders or a GOTO activity; this implementation has no queued native GOTO activity, so those cached goals are cleared after validation. Advisor worker-role markers and pending Senate war targets are transient. Persisted strategic mission maps remain a separate application feature; exact native task/load decision equivalence is not established.

Government goals use source player bonuses, city-specific government effects, technology amortization, and the 5–20-turn reevaluation cache. Threat arrival assessment includes embarked troops, amphibious disembarkation differences, and paradrop reach. Embassy clauses establish permanent directional access; ordinary physical contact expires, with source timing for initial contact and renewals. Stationary attacks and bombardments do not renew contact. Ferry availability and passenger leadership are retained independently of current cargo, including release on death and ownership changes.

Diplomatic intelligence retains the reference's enemy-alliance, ally-at-war, and shared-ally snapshots. A new phase applies love changes using the previous snapshot before refreshing intelligence for negotiations and war obligations. The snapshot is transient and is recomputed on campaign load without love changes or random draws. City output advisor priorities preserve the source integer science priority of 21 and its `adv_want` floating-point accumulation. Happiness wants use individual effects, citizen stages, and effect ranges; health improvements participate separately.

Government candidate evaluation rearranges city workers and specialists using the reference citizen manager's output factors, surplus minima, tile-type prerequisites, and four fallback stages. Its branch-and-bound search follows native estimated-type ordering, tile ordering, strict fitness ties, componentwise production bounds, learned luxury pruning, and the 27,500-iteration budget. It retains the best incumbent when that budget is exhausted, rather than substituting an uncapped optimizer. Government wants preserve source `float` accumulation. Tests cover small-city enumeration, native traversal steps, early and late search aborts, and emergency arithmetic.

These changes do not establish complete AI equivalence. The effect handling remains specific to supported Civ2Civ3 effects rather than a complete generic ruleset evaluator. Bodyguard requests, typed unit/city assignments, rendezvous, movement synchronization, and ferry capacity/waiting have source-based implementations. Military advice applies native danger, urgency, martial-law and veteran-building replacement priorities. Normal/Civ2Civ3 tax management follows reserve, science, war and future-technology priorities; 64 fixture vectors come from compiling the native C algorithm. Government and tax changes precede emergency citizen allocation, city advice, purchases and research. The normal AI GOTO planner implements native path scopes, action endpoints, ferry overlap, risk costs and queue ties. Aircraft mission and emergency-rescue paths include fuel constraints, return trips and refueling waits. City founding, growth, population loss and ownership changes invoke the citizen manager; unrelated manual assignments remain in place. City activity uses native player-city list order for nationality conversion, then random selection with swap-removal. Production defender selection now follows static native role priorities independently of trained tactical counter forecasts. Standalone nuclear and cruise missiles follow the normal air manager, including its strict movement-cost cutoff; hunter-carried missiles use the separate source launch path. Carriers delegate to their actual cargo boss and retain the source done/hold behavior instead of the former custom sortie-range positioning. New games continue the post-start native random stream, verified against 720 literal C-oracle vectors. CivCraft maps its displayed 64-bit seed to 32 bits by XOR-folding the two halves, preserving existing map generation. This does not reproduce every Freeciv startup draw: player shuffling, identifiers, assignment and optional setup still need a separate audit. Defensive-building valuation now includes the native capital, continent, ocean and invasion terms, with separate effect rows accumulated literally. Static defensive research feedback uses native role scoring and production gates. The supported simple military cache follows native type membership and order. Offensive aircraft production applies the nominal HP-loss eligibility gate independently of tactical use of existing aircraft and defensive unit roles. Shared city danger now uses the native dangerous-player predicate, including low-trust peace partners and ceasefires. Confirmed remaining differences include other threat/knowledge-policy consumers, domestic technology-effect valuation, other advisor cache timing, historical fog extras, further activity/task lifecycles, and incomplete auditing of all native list consumers. The unit catalog now contains all 57 reference types, preserving the original 42 serialized enum discriminants and using a separate native ruleset order. Production, requirements, direct upgrade chains, AI role lists and sprites include the added types. This does not establish full special-unit behavior: Leader loss conditions, Storm initialization/random movement, and broader dual-role phase scheduling still require dedicated audits. Complete-match random-draw equivalence is not established. Tests exercise specific rules and scenarios, rather than proving identical full-match decisions or random draw order.

Hypothetical unit upkeep uses the native requirement context: without an actual unit, `HasHomeCity` is unknown and the fixed ruleset’s gold-upkeep effects fail the certain check. Actual homed units still incur their normal support bill. Advisor eligibility therefore does not substitute actual-unit upkeep for the source type query.

End-turn processing restores each player’s Workshop upgrades, hit points, emergency aircraft returns and fuel before that player’s city activities and national upkeep/research. Work progress, fortification and MP refill occur at the next local phase after disasters, climate and calendar updates. Fortify has zero base MP cost; an exhausted unit makes no fortification progress until a phase with remaining MP. Hit-point restoration clears the old movement flag, while movement during emergency rescue remains recorded through MP refill. A Barracks or Workshop completed in cities therefore cannot act retroactively during that phase’s preceding restoration. Revolution completion is checked at both native end-phase and next-phase points. The combined restoration/activity helper remains a standalone scenario convenience; live turns use the split APIs. Unit activity processing completes projects immediately after the acting unit refills movement, before processing the next unit in native player-list order. Road completion therefore affects later units’ movement refill, and completion clears matching coworkers before they can progress or consume promotion draws. Remaining old movement grants the full activity rate, even when only a fragment remains. Native worker promotion eligibility is independent of remaining movement; single-level and non-worker types skip the draw, while an eligible zero-percent Transport work promotion still consumes one draw.

Shared city production includes Fundamentalism tithes before commercial bonuses, Harbour food, local Pyramids shields, Federation center luxury, Highways/Stock Exchange bonuses, and distance-based food waste. Celebration uses the existing saved happy-history flag and a separate rapture counter; output uses the reference cached-refresh timing. Actual-unit pathfinding and AI forecasts use damage-scaled movement rates, whole-move class minima, and movement effects after damage. AI movement reveals terrain immediately before contact and village outcomes.

The source NonMil flag is tracked independently of attack strength: civilians may cross peaceful territory, while a zero-attack Carrier remains military for border restrictions and martial law. Foreign non-allied stacks and cities remain excluded. FieldUnit bombers and nuclear weapons impose happiness upkeep even in friendly cities and territory. Movement shares enemy-infrastructure restrictions, river road costs by class, wheeled-unit terrain nativity, boarding costs, and explicit ZOC actor/generator flags. Mountain movement penalties and river defense attributes are separate from ordinary terrain entry costs and terrain defense. Leader threat selection keeps normal reverse pathfinding; the selected aircraft enemy's escape-distance map uses the native fuel engine, including its two queues, refueling waits, cached field conversions, and attack-return conditions.

Unit obsolescence and upgrades traverse the complete native successor chain, including available later successors whose intermediate technologies are absent. Future-build eligibility deliberately ignores the candidate’s current government and technology requirements, while retaining NoBuild, existing obsolescence and native city access checks. Fanatics require Fundamentalism and Guerilla Warfare to produce, cost population, can join a city, and have no upkeep under Fundamentalism. Upkeep uses source type costs and government output effects; a Caravel’s lack of ShieldToGold does not bypass a government’s zero shield-upkeep multiplier. CityBuster firepower, Big Land nativity/terrain rules, Marine amphibious attack flags, helicopter field access, Fortress construction, bribery immunity and actual veteran-level limits use unit metadata.

Normal AI follows the reference H_TARGETS production gate: dedicated hunter hull and hunter-ammunition advice are suppressed even when a worthwhile target exists. Existing Hunter-role units still hunt, including land units; they scan the default PF map in native order, use the first qualifying tile/stack, compare nominal type speeds, include GameLoss target bonuses, and execute the scanned default-PF path. Hard-profile dedicated hunter production is not implemented. Interception snapshots now retain land and sea units, but the local EndTurn snapshot timing is not proven equivalent to native per-unit phase timing.

Transport carries the native Workers flag. It can construct Deep Ocean oil platforms with Miniaturization and clean ocean contamination; it cannot perform Sea-excluded irrigation, ordinary mining, planting or cultivation. Worker planning uses sea movement/nativity and the correct unknown-tile cost. Native worker precedence also applies to standalone AI Transport dispatch before the ferry branch. The reference's own-passenger skip and lack of worker-phase passenger activation can leave such a dual-role Transport holding its cargo and booking; generic delivery regressions use a true ferry such as Galleon. Existing global fleet/assault prepasses remain a separate phase-order audit item.

Hunter target and previous-position maps retain their existing map wire representation but serialize entries by unit identifier. This removes randomized HashMap bucket order from save/replay byte comparisons, including the expanded all-unit interception history, without adding a save header or changing map payload fields.

AI research wants are retained per-player f32 phase snapshots. New and loaded campaigns start with zero wants; last activities clear the preceding snapshot, add government feedback before taxes, then building and ordered military feedback during city management, followed by technology-effect feedback before research selection. First-activity threat assessment can add counter feedback before unit movement. Quotes and selection read the retained snapshot instead of recomputing it after every state change. Civil-war player copies inherit their parent's vector, while save/load deliberately does not retain it. Explicit read-only previews are used only for isolated advisor scenarios. Every integer-plus-float research assignment truncates at its native boundary; goal and per-city division stays integer, and technology prices divide float wants before integer conversion.

Threat-counter research now uses daitech.c:dai_wants_defender_against, including attacker-specific defense multipliers, direct buildability, future eligibility, strict cheapest-goal ties and the integer half-value boundary. The pure selector is checked against 128 vectors compiled from the supplied C implementation. Offensive production and future-unit research use a shared native target/process evaluator: sea then land seeds, Normal known-target policy, actual or virtual ferries, combined ferry search and beachheads, type-based defender forecasts, reinforcement costs, missing technology cost, travel/unhappiness penalties and balanced-cost amortization. Feedback is applied as ordered events rather than grouped per-tech totals. Native quirks involving retained empty-city defender types and later-candidate ferry travel time are preserved. Ongoing native GOTO activity and average-production cache timing still depend on shared local representations; this does not establish identical full-match choices.

Raw military unhappiness is shared by city feelings and offensive advice. Police Stations increase the free military-content allowance instead of reducing each unit's cost. Government factors and allowances, Women's Suffrage, Statue of Zeus, King Richard's Crusade, FieldUnit behavior and base-class aggression follow the supplied effects and city_unit_unhappiness rules. Raw pressure is evaluated before citizen-feeling saturation. No bundled base has NoStackDeath: under the default killstack setting, only cities protect a stack, and offensive victim counts use that same rule as combat.

Research goals now have an explicit UNSET state distinct from Future Tech. Existing real and Future indices are unchanged; only a goal may store the new sentinel. Matching goals clear when their technology is acquired, and all-negative advisor goals can clear too. The research UI falls back to the current project without drawing a nonexistent node. Save validation rejects the sentinel as an active or known technology, and commands reject it as an explicit human research choice. The existing automatic valid-current-target behavior is retained; the reference's inactive active-research state remains unsupported.

## Configurable wrapping

New games default to Freeciv's `WRAPX` cylinder option; `--wrap xy` retains the
reference Civ2Civ3 preset's `WRAPX|WRAPY` torus. `MapWrap` is per campaign and is
stored in new saves. Both modes currently use the existing 32×64 explicit map
dimensions, rather than changing the size/aspect ratio when changing wrapping.
Cylinder generation follows the reference's non-wrapped height-map boundary
initialization and subdivision, singular-edge handling, linear north/south
latitude, bounded neighbor iteration and zero-edge start-position smoothing.
Torus exact-seed reference oracles remain unchanged.

Source: `server/settings.c:1609`, `common/map.c:1565`,
`server/generator/height_map.c:115–267`,
`server/generator/mapgen_utils.c:186–236`. Gameplay distances, neighbors,
worker/city radii, visibility and pathfinding share the campaign geometry.
Cylinder camera bounds now use the native normal-tile scroll window and
viewport midpoint, including oversized-view expansion; torus
viewports, fog, artwork neighbors and route/animation seams wrap both axes.
