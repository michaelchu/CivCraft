# Mechanics source sheet

This sheet is the contract for the values used by the Rust build. Paths beginning with reference/freeciv are relative to the supplied target. Readable ruleset and C-source values are the primary evidence, and an out-of-tree arm64 build of the original server was also traced to verify that the compiled control flow uses the same rules.

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
| Topology | `_set_sizes` | `0x100031f5c` | Isometric dimensions are even and the native Y dimension is doubled, producing 32 by 64 for this 2,000-tile request. |
| Generation | `_map_fractal_generate` | `0x10001fef4` | The standard map path uses fractal generation. |
| Generation | `_make_land` | `0x10002437c` | Land generation is driven by the configured land percentage. |
| Distance | `_map_distance_vector` | `0x100395e70` | Wrapped native coordinates are converted before distance is measured. |
| Distance | `_base_map_distance_vector` | `0x10039859c` | Topology-aware map vectors choose the shortest wrapped displacement. |
| Distance | `_map_vector_to_real_distance` | `0x100395c10` | Isometric-hex distance uses six legal directions. |
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
| Recovery | `_hp_gain_coord` | `0x1003fee50` | City minimum HP, base regeneration, and fortified regeneration are cumulative. |
| Recovery | `_unit_gain_hitpoints` | `0x1003fef90` | A unit that moved does not receive normal turn regeneration. |
| Recovery | `_unit_restore_hitpoints` | `0x100190388` | End-of-turn healing is capped at the unit type's maximum HP. |
| AI | `_dai_military_attack` | `0x100458244` | Military units evaluate and move toward attack targets. |
| AI | `_manage_auto_explorer` | `0x100013bcc` | Automated explorers seek reachable unexplored tiles. |
| AI | `_auto_worker_findwork` | `0x100018c20` | Automated workers search for legal terrain improvements. |
| AI | `_dai_do_build_city` | `0x1004473c4` | AI settlers validate and found at candidate city sites. |
| AI | `_advisor_choose_build` | `0x10000d468` | City production is selected through the advisor path. |

Every function listed above returned from `native_function_pseudocode`. The Ghidra bridge returned no result for them, so Craft Anything's built-in lifter supplied register-level pseudo-C instead. That output has approximate types and conditions, unresolved instructions, and some unstructured branches; function identities, addresses, calls, and strings were used as native confirmation, while exact values and semantics were cross-checked against the supplied C source and Civ2Civ3 rulesets.

## Match

| Value used | Number or rule | Source |
| --- | --- | --- |
| Ruleset | Civ2Civ3 | reference/freeciv/common/game.h, GAME_DEFAULT_RULESETDIR |
| Players | 6 total: one person and five computer rivals | reference/freeciv/data/civ2civ3/game.ruleset, aifill = 6 |
| Source map target | 100 land tiles per player | reference/freeciv/data/civ2civ3/game.ruleset, tilesperplayer = 100 |
| Build map | 32 by 64 wrapped native tiles, 2,048 total | `generator_init_topology` and `set_sizes`: six players × 100 land tiles × 100 / 30% land = 2,000 requested tiles; the 1:1 isometric torus formula rounds to 32 × 64 |
| Build land | 614 tiles | 30% of the 2,048 generated tiles, matching the configured land percentage to the nearest whole tile |
| Generator | Deterministic fractal-like continents | Native `map_fractal_generate` and `make_land` confirm the source path; the compact Rust height function is an estimated substitute |
| Wrap | Horizontal and vertical | reference/freeciv/data/civ2civ3/game.ruleset, WRAPX and WRAPY |
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

Terrain entry cost is paid when a land unit enters the destination. As in Freeciv, a unit with movement remaining may enter a more expensive rough tile and finish at zero. Water blocks the opening land units. Native/map coordinate conversion and shortest wrapped vectors are used for movement, visibility, city range, AI targets, and pathfinding, so crossing either edge remains one legal hex step.

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
- `_hp_gain_coord` at `0x1003fee50` calculates `max(max HP × HP_Regen / 100, max HP × Min_HP_Pct / 100) + ceil(max HP / 10) × HP_Regen_2 / 10`. Opening ground units have 0% `HP_Regen`, a universal 10-point `HP_Regen_2`, a city-center `Min_HP_Pct` of 33, and another 10 points of `HP_Regen_2` while fortified. A damaged 10-HP unit therefore gains 1 HP in the field, 4 HP in a city, or 5 HP while fortified in a city. `_unit_gain_hitpoints` at `0x1003fef90` suppresses normal regeneration after movement, and `_unit_restore_hitpoints` at `0x100190388` caps healing at maximum HP.
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

The city center produces at least one food, shield, and trade, and each citizen works the best currently available tile inside radius squared 5. This compact build automatically chooses tiles instead of reproducing the full citizen governor dialog. `_city_build_unit` at `0x1000edbe8` completes a unit only after the city has its shield cost and, for population-cost units, `city size > population cost`; completion subtracts both costs. `_city_change_production_penalty` at `0x100376488` keeps all accumulated shields when switching from one unit target to another because both targets share the unit production class. `_city_build_building` at `0x1000ed3f4` is a distinct path, and buildings and wonders remain outside this focused slice.

## Controls

Arrow and numeric direction movement, B Build City, G Go to Tile, X Auto Explore, A Auto Worker, S Sentry, F Fortify, M Mine, I Irrigate, R Road, W Wait/Space Done Moving, and Shift+Enter End Turn follow reference/freeciv/client/gui-qt/shortcuts.cpp, client/gui-gtk-4.0/gui_main.c, and client/gui-sdl3/menu.c. Mouse selection, right-click goto, middle-drag pan, minimap recentering, and wheel zoom follow the role of the source clients; exact mouse gestures differ among those clients.

## Computer players

The source default AI separately assesses city danger, chooses defenders and attackers, evaluates city-founding sites, automates workers, and sends idle units exploring in reference/freeciv/ai/default. The Rust skirmish preserves those priorities with a smaller deterministic planner:

1. Found the first city immediately on a legal starting tile.
2. Keep a military defender in each city.
3. Improve owned land with roads, irrigation, then mines.
4. Produce another Settlers unit once a city reaches size 3, up to three cities.
5. Explore unknown land with Explorers and idle nonmilitary units.
6. Send surplus military units toward the nearest rival city and attack.

The priority order is source-derived. Its reduced search depth and deterministic tie-breaking are estimated for this focused build. AI gets no resource or combat bonus, corresponding to normal rather than cheating difficulty.
