# Scenario boundary and coverage migration

The registered gameplay catalog is `src/scenarios/definitions/mod.rs`. Scenario
definitions own their setup composition, actions, observations and named checks. The headless
runner owns tracing, analytics, turn advancement and report output. `sim` owns the
rules, state, typed fixture construction and read-only queries.

`ScenarioRun` is the common lifecycle for both immediate scripted actions and
normal-turn observations. A script may request preparatory turns; the runner executes
them within the action boundary. This preserves diplomacy-parity's version, checks
and report provenance without advancing the turn inside scenario code.

World construction is under `sim/fixtures`: `world`, `cities`, `units`,
`diplomacy`, `economy`, `research` and `readiness` hold typed preparation APIs.
`sim/fixtures.rs` only owns the builder lifecycle and shared imports. Blank terrain
presets remain reusable simulation fixtures. Fixed rosters, coordinates, research
weights and funding arrangements live with the scenario that chooses them.
Strategy and funding unit tests reuse those setup functions through test-only
exports; production simulation code does not depend on scenario definitions.

The optional catalog, its constructors and every registered definition share one
module gate on `scenarios::definitions`. The runtime lifecycle and `generated-ai`
remain available without the `scenarios` feature. Simulation-only setup adapters
retain gates at their owning module or implementation boundary where necessary.

UI review's simulation setup is under `sim/ui_review`; rendering, input and
screenshot capture remain with the client.

## Coverage decisions

Every previously registered scenario keeps its CLI ID, registration order, named
checks and provenance version. Their definitions now live under
`src/scenarios/definitions`; the intermediate catalog wrappers and simulation-local
check implementations are removed. Culture/history coverage is additionally registered
as `culture-history`.

The old test-only helper tree is now `sim/tests/gameplay`, with scoped private-rule
helpers in the owning subsystem's `unit_cases` module. These are ordinary Rust
unit tests, compiled only under `cfg(test)`: they have no registry, runtime lifecycle,
or production runner. Their setup methods use fixture names; other test helpers
use test names. Private RNG vectors, arithmetic, caches and list-order assertions
retain internal access in those tests rather than requiring a public scenario API.

Duplicate unit tests that merely called an already registered scenario were absorbed
by the common catalog test (strategy, funding, relationship memory, healing, ferry,
and airborne-city cases). Their assertions remain in the direct scenario definitions.
The lifecycle overflow unit test now checks production and loss explicitly, while
the registered scenario retains the complete founding/conquest/trade/disaster sequence.

## Validation

Validated after the refactor:

- Formatting and Clippy (`--all-targets -- -D warnings`) pass.
- `cargo test --offline --all-targets`: 1,308 passed, one ignored.
- `cargo test --offline --doc`: passes (zero runnable doc tests).
- Headless builds pass with the scenario feature enabled and disabled. The
  disabled-feature CLI lists only `generated-ai`; the enabled dependency graph
  excludes desktop and audio packages.
- The desktop binary builds with `desktop-audio`; it also compiles with the
  scenario catalog disabled. This Linux environment needed a temporary linker
  alias under `/tmp` to its installed `libasound.so.2`, because the development
  linker name `libasound.so` was absent. No repository linker workaround was added.
- All 30 registered scenarios pass in the release headless build at seed 42,
  including all 29 original IDs and the new `culture-history`. Runs verify each
  named check, summary totals and provenance version. Normal runs use one turn,
  except `campaign-state-presets` (two) and `naval-ai-assault` (its 30-turn deadline).
- The catalog/fixture organization follow-up compares all 30 scenarios against
  the preceding release binary at seed 42. Reports match after excluding elapsed
  time and working-tree status; all event, change, snapshot and shared-value
  sidecars match byte for byte. The full Rust suite, Clippy, documentation tests
  and feature-enabled/disabled builds also pass after that follow-up.
- Retained unit helpers were compared against their original implementations:
  95 bodies match after identifier/formatting changes; two differ only in renamed
  fixture panic labels. Their ordinary Rust tests also pass.
- UI screenshot review was skipped because the configured Civ III artwork
  prerequisite was unavailable. No visual baseline was generated or changed.

## Helper disposition

The source/name columns identify the original implementation; the disposition names
its current destination or explains its retained unit-test responsibility. Setup-only
helpers are fixtures, not an additional execution system.

| Original source | Original helper | Disposition |
| --- | --- | --- |
| `src/sim/advisors/advisor_production.rs` | `expansion_balance_playtest` | Migrated to `src/scenarios/definitions/expansion_balance.rs`; existing coverage preserved |
| `src/sim/ai_economy/funding/scenarios.rs` | `wartime_funding_scenario` | Migrated directly to `src/scenarios/definitions/wartime_funding.rs`; checks preserved in catalog coverage |
| `src/sim/ai_strategy/scenarios.rs` | `shared_strategy_scenario` | Migrated directly to `src/scenarios/definitions/shared_strategy.rs`; checks preserved in catalog coverage |
| `src/sim/diplomacy/relationships/scenarios.rs` | `relationship_memory_scenario` | Migrated directly to `src/scenarios/definitions/relationship_memory.rs`; checks preserved in catalog coverage |
| `src/sim/diplomacy/scenarios.rs` | `diplomacy_playtest_friendship` | Fixture: `src/sim/diplomacy/unit_cases.rs::diplomacy_test_friendship`; private treaty valuations, directional memory and observer timers |
| `src/sim/diplomacy/scenarios.rs` | `diplomacy_playtest_alliance_offer_setup` | Fixture: `src/sim/diplomacy/unit_cases.rs::diplomacy_test_alliance_offer_setup`; private treaty valuations, directional memory and observer timers |
| `src/sim/diplomacy/scenarios.rs` | `diplomacy_playtest_allied_trade_baseline` | Fixture: `src/sim/diplomacy/unit_cases.rs::diplomacy_test_allied_trade_baseline`; private treaty valuations, directional memory and observer timers |
| `src/sim/diplomacy/scenarios.rs` | `diplomacy_playtest_allied_sharing` | Fixture: `src/sim/diplomacy/unit_cases.rs::diplomacy_test_allied_sharing`; private treaty valuations, directional memory and observer timers |
| `src/sim/diplomacy/scenarios.rs` | `ai_treaty_policy_playtest` | Unit check: `src/sim/diplomacy/unit_cases.rs::ai_treaty_policy_test`; private treaty valuations, directional memory and observer timers |
| `src/sim/diplomacy/scenarios.rs` | `ai_diplomatic_intelligence_playtest` | Unit check: `src/sim/diplomacy/unit_cases.rs::ai_diplomatic_intelligence_test`; private treaty valuations, directional memory and observer timers |
| `src/sim/diplomacy/scenarios.rs` | `ai_observer_incident_cadence_playtest` | Unit check: `src/sim/diplomacy/unit_cases.rs::ai_observer_incident_cadence_test`; private treaty valuations, directional memory and observer timers |
| `src/sim/diplomacy/scenarios.rs` | `ai_diplomatic_love_playtest` | Unit check: `src/sim/diplomacy/unit_cases.rs::ai_diplomatic_love_test`; private treaty valuations, directional memory and observer timers |
| `src/sim/diplomacy/scenarios.rs` | `ai_space_strategy_playtest` | Unit check: `src/sim/diplomacy/unit_cases.rs::ai_space_strategy_test`; private treaty valuations, directional memory and observer timers |
| `src/sim/diplomacy/scenarios.rs` | `city_clause_playtest` | Unit check: `src/sim/diplomacy/unit_cases.rs::city_clause_test`; private treaty valuations, directional memory and observer timers |
| `src/sim/diplomacy/scenarios.rs` | `treaty_map_playtest` | Unit check: `src/sim/diplomacy/unit_cases.rs::treaty_map_test`; private treaty valuations, directional memory and observer timers |
| `src/sim/diplomacy/scenarios.rs` | `city_clause_bundle_playtest` | Unit check: `src/sim/diplomacy/unit_cases.rs::city_clause_bundle_test`; private treaty valuations, directional memory and observer timers |
| `src/sim/diplomacy/trade_policy/scenarios.rs` | `proactive_trade_scenario` | Migrated directly to `src/scenarios/definitions/proactive_trade.rs`; checks preserved in catalog coverage |
| `src/sim/headless_scenarios.rs` | `capture_known_tiles` | Setup-only helper in `src/sim/fixtures/world.rs` |
| `src/sim/headless_scenarios.rs` | `give_unit_full_movement` | Setup-only helper in `src/sim/fixtures/units.rs` |
| `src/sim/headless_scenarios.rs` | `ai_paradrop_playtest` | Migrated to `src/scenarios/definitions/ai_paradrops.rs`; existing coverage preserved |
| `src/sim/headless_scenarios.rs` | `great_leader_playtest` | Direct definition in `src/scenarios/definitions/great_leader.rs`; named checks preserved |
| `src/sim/headless_scenarios.rs` | `city_healing_playtest` | Direct definition in `src/scenarios/definitions/unit_recovery.rs`; named checks preserved |
| `src/sim/headless_scenarios.rs` | `settlement_ferry_production_playtest` | Migrated to `src/scenarios/definitions/settlement_ferry.rs`; existing coverage preserved |
| `src/sim/headless_scenarios.rs` | `freeciv_combat_matrix_playtest` | Migrated to `src/scenarios/definitions/combat_matrix.rs`; existing coverage preserved |
| `src/sim/lifecycle.rs` | `lifecycle_history_playtest` | Migrated to `src/scenarios/definitions/lifecycle_history.rs`; existing coverage preserved |
| `src/sim/playtest/ai_scenarios.rs` | `ai_reserve_playtest` | Unit check: `src/sim/tests/gameplay/ai_tests.rs::ai_reserve_test`; private AI role selection, assignment and reserve calculations |
| `src/sim/playtest/ai_scenarios.rs` | `civilian_upgrade_roster_playtest` | Unit check: `src/sim/tests/gameplay/ai_tests.rs::civilian_upgrade_roster_test`; private AI role selection, assignment and reserve calculations |
| `src/sim/playtest/ai_scenarios.rs` | `ai_ground_invasion_playtest` | Unit check: `src/sim/tests/gameplay/ai_tests.rs::ai_ground_invasion_test`; private AI role selection, assignment and reserve calculations |
| `src/sim/playtest/ai_scenarios.rs` | `ai_aircraft_advisor_playtest` | Unit check: `src/sim/tests/gameplay/ai_tests.rs::ai_aircraft_advisor_test`; private AI role selection, assignment and reserve calculations |
| `src/sim/playtest/ai_scenarios.rs` | `nuclear_ai_role_playtest` | Unit check: `src/sim/tests/gameplay/ai_tests.rs::nuclear_ai_role_test`; private AI role selection, assignment and reserve calculations |
| `src/sim/playtest/ai_scenarios.rs` | `ai_threat_route_cost_playtest` | Unit check: `src/sim/tests/gameplay/ai_tests.rs::ai_threat_route_cost_test`; private AI role selection, assignment and reserve calculations |
| `src/sim/playtest/ai_scenarios.rs` | `ai_idle_exploration_playtest` | Unit check: `src/sim/tests/gameplay/ai_tests.rs::ai_idle_exploration_test`; private AI role selection, assignment and reserve calculations |
| `src/sim/playtest/ai_scenarios.rs` | `ai_exploration_legality_playtest` | Unit check: `src/sim/tests/gameplay/ai_tests.rs::ai_exploration_legality_test`; private AI role selection, assignment and reserve calculations |
| `src/sim/playtest/ai_scenarios.rs` | `ai_exploration_playtest` | Unit check: `src/sim/tests/gameplay/ai_tests.rs::ai_exploration_test`; private AI role selection, assignment and reserve calculations |
| `src/sim/playtest/aviation_scenarios.rs` | `nuclear_playtest_setup` | Fixture: `src/sim/tests/gameplay/aviation_tests.rs::nuclear_fixture`; unit-level fuel, transport and action-legality fixtures |
| `src/sim/playtest/aviation_scenarios.rs` | `cruise_missile_playtest_setup` | Fixture: `src/sim/tests/gameplay/aviation_tests.rs::cruise_missile_fixture`; unit-level fuel, transport and action-legality fixtures |
| `src/sim/playtest/aviation_scenarios.rs` | `aegis_playtest_setup` | Fixture: `src/sim/tests/gameplay/aviation_tests.rs::aegis_fixture`; unit-level fuel, transport and action-legality fixtures |
| `src/sim/playtest/aviation_scenarios.rs` | `sam_battery_playtest_setup` | Fixture: `src/sim/tests/gameplay/aviation_tests.rs::sam_battery_fixture`; unit-level fuel, transport and action-legality fixtures |
| `src/sim/playtest/aviation_scenarios.rs` | `stealth_aircraft_playtest_setup` | Fixture: `src/sim/tests/gameplay/aviation_tests.rs::stealth_aircraft_fixture`; unit-level fuel, transport and action-legality fixtures |
| `src/sim/playtest/aviation_scenarios.rs` | `carrier_playtest_setup` | Fixture: `src/sim/tests/gameplay/aviation_tests.rs::carrier_fixture`; unit-level fuel, transport and action-legality fixtures |
| `src/sim/playtest/aviation_scenarios.rs` | `bombard_playtest_setup` | Fixture: `src/sim/tests/gameplay/aviation_tests.rs::bombard_fixture`; unit-level fuel, transport and action-legality fixtures |
| `src/sim/playtest/aviation_scenarios.rs` | `airlift_playtest_setup` | Fixture: `src/sim/tests/gameplay/aviation_tests.rs::airlift_fixture`; unit-level fuel, transport and action-legality fixtures |
| `src/sim/playtest/aviation_scenarios.rs` | `ai_aircraft_advisor_playtest_setup` | Fixture: `src/sim/tests/gameplay/aviation_tests.rs::ai_aircraft_advisor_fixture`; unit-level fuel, transport and action-legality fixtures |
| `src/sim/playtest/border_scenarios.rs` | `fogged_border_playtest` | Unit check: `src/sim/tests/gameplay/border_tests.rs::fogged_border_test`; internal border-strength and observed-map assertions |
| `src/sim/playtest/border_scenarios.rs` | `city_borders_playtest` | Unit check: `src/sim/tests/gameplay/border_tests.rs::city_borders_test`; internal border-strength and observed-map assertions |
| `src/sim/playtest/city_economy_scenarios.rs` | `ai_government_playtest` | Unit check: `src/sim/tests/gameplay/city_economy_tests.rs::ai_government_test`; private yield formulas, workforce allocation and city-manager policy |
| `src/sim/playtest/city_economy_scenarios.rs` | `barbarian_output_waste_playtest` | Unit check: `src/sim/tests/gameplay/city_economy_tests.rs::barbarian_output_waste_test`; private yield formulas, workforce allocation and city-manager policy |
| `src/sim/playtest/city_economy_scenarios.rs` | `city_output_rules_playtest` | Unit check: `src/sim/tests/gameplay/city_economy_tests.rs::city_output_rules_test`; private yield formulas, workforce allocation and city-manager policy |
| `src/sim/playtest/city_economy_scenarios.rs` | `industrial_output_playtest` | Unit check: `src/sim/tests/gameplay/city_economy_tests.rs::industrial_output_test`; private yield formulas, workforce allocation and city-manager policy |
| `src/sim/playtest/city_economy_scenarios.rs` | `city_worker_lifecycle_playtest` | Unit check: `src/sim/tests/gameplay/city_economy_tests.rs::city_worker_lifecycle_test`; private yield formulas, workforce allocation and city-manager policy |
| `src/sim/playtest/city_economy_scenarios.rs` | `ai_city_manager_playtest` | Unit check: `src/sim/tests/gameplay/city_economy_tests.rs::ai_city_manager_test`; private yield formulas, workforce allocation and city-manager policy |
| `src/sim/playtest/combat_scenarios.rs` | `combat_population_playtest` | Unit check: `src/sim/tests/gameplay/combat_tests.rs::combat_population_test`; population-loss rules and internal citizen rearrangement |
| `src/sim/playtest/culture_scenarios.rs` | `culture_accumulation_playtest` | Migrated to `src/scenarios/definitions/culture_history.rs`; existing coverage preserved |
| `src/sim/playtest/diplomacy_scenarios.rs` | `map_snapshot_playtest` | Unit check: `src/sim/tests/gameplay/diplomacy_tests.rs::map_snapshot_test`; directional contact, knowledge and treaty-state fixtures |
| `src/sim/playtest/diplomacy_scenarios.rs` | `diplomacy_playtest_tick` | Unit check: `src/sim/tests/gameplay/diplomacy_tests.rs::diplomacy_test_tick`; directional contact, knowledge and treaty-state fixtures |
| `src/sim/playtest/diplomacy_scenarios.rs` | `diplomacy_playtest_technologies` | Unit check: `src/sim/tests/gameplay/diplomacy_tests.rs::diplomacy_test_technologies`; directional contact, knowledge and treaty-state fixtures |
| `src/sim/playtest/diplomacy_scenarios.rs` | `diplomacy_playtest_allied_exchange` | Unit check: `src/sim/tests/gameplay/diplomacy_tests.rs::diplomacy_test_allied_exchange`; directional contact, knowledge and treaty-state fixtures |
| `src/sim/playtest/diplomacy_scenarios.rs` | `diplomacy_playtest_unit` | Unit check: `src/sim/tests/gameplay/diplomacy_tests.rs::diplomacy_test_unit`; directional contact, knowledge and treaty-state fixtures |
| `src/sim/playtest/diplomacy_scenarios.rs` | `civil_war_playtest` | Unit check: `src/sim/tests/gameplay/diplomacy_tests.rs::civil_war_test`; directional contact, knowledge and treaty-state fixtures |
| `src/sim/playtest/endgame_scenarios.rs` | `multicultural_playtest_setup` | Fixture: `src/sim/tests/gameplay/endgame_tests.rs::multicultural_fixture`; controlled spaceship and achievement fixtures |
| `src/sim/playtest/endgame_scenarios.rs` | `spaceship_playtest_setup` | Fixture: `src/sim/tests/gameplay/endgame_tests.rs::spaceship_fixture`; controlled spaceship and achievement fixtures |
| `src/sim/playtest/list_order_scenarios.rs` | `native_list_order_playtest` | Unit check: `src/sim/tests/gameplay/list_order_tests.rs::native_list_order_test`; private player/tile/home/cargo ordering invariants |
| `src/sim/playtest/list_order_scenarios.rs` | `native_cargo_order_playtest` | Unit check: `src/sim/tests/gameplay/list_order_tests.rs::native_cargo_order_test`; private player/tile/home/cargo ordering invariants |
| `src/sim/playtest/list_order_scenarios.rs` | `native_city_activity_order_playtest` | Unit check: `src/sim/tests/gameplay/list_order_tests.rs::native_city_activity_order_test`; private player/tile/home/cargo ordering invariants |
| `src/sim/playtest/military_ai/advisors.rs` | `offensive_research_playtest` | Unit check: `src/sim/tests/gameplay/military_ai/advisors.rs::offensive_research_test`; private advisor wants, defender selection and threat scoring |
| `src/sim/playtest/military_ai/advisors.rs` | `counter_defender_research_playtest` | Unit check: `src/sim/tests/gameplay/military_ai/advisors.rs::counter_defender_research_test`; private advisor wants, defender selection and threat scoring |
| `src/sim/playtest/military_ai/advisors.rs` | `native_defensive_effects_playtest` | Unit check: `src/sim/tests/gameplay/military_ai/advisors.rs::native_defensive_effects_test`; private advisor wants, defender selection and threat scoring |
| `src/sim/playtest/military_ai/advisors.rs` | `native_city_defense_advisor_playtest` | Unit check: `src/sim/tests/gameplay/military_ai/advisors.rs::native_city_defense_advisor_test`; private advisor wants, defender selection and threat scoring |
| `src/sim/playtest/military_ai/advisors.rs` | `ai_growth_expansion_playtest` | Unit check: `src/sim/tests/gameplay/military_ai/advisors.rs::ai_growth_expansion_test`; private advisor wants, defender selection and threat scoring |
| `src/sim/playtest/military_ai/air.rs` | `ai_paradrop_stack_playtest` | Unit check: `src/sim/tests/gameplay/military_ai/air.rs::ai_paradrop_stack_test`; aircraft range, action legality and internal assignment invariants |
| `src/sim/playtest/military_ai/air.rs` | `paradrop_playtest_setup` | Fixture: `src/sim/tests/gameplay/military_ai/air.rs::paradrop_fixture`; aircraft range, action legality and internal assignment invariants |
| `src/sim/playtest/military_ai/air.rs` | `ai_fighter_playtest` | Unit check: `src/sim/tests/gameplay/military_ai/air.rs::ai_fighter_test`; aircraft range, action legality and internal assignment invariants |
| `src/sim/playtest/military_ai/air.rs` | `ai_embarked_paradrop_danger_playtest` | Unit check: `src/sim/tests/gameplay/military_ai/air.rs::ai_embarked_paradrop_danger_test`; aircraft range, action legality and internal assignment invariants |
| `src/sim/playtest/military_ai/ground.rs` | `ai_counterattack_playtest` | Unit check: `src/sim/tests/gameplay/military_ai/ground.rs::ai_counterattack_test`; private guard/assault assignments and path decisions |
| `src/sim/playtest/military_ai/ground.rs` | `ai_reinforcements_playtest` | Unit check: `src/sim/tests/gameplay/military_ai/ground.rs::ai_reinforcements_test`; private guard/assault assignments and path decisions |
| `src/sim/playtest/military_ai/ground.rs` | `ai_fortifications_playtest` | Unit check: `src/sim/tests/gameplay/military_ai/ground.rs::ai_fortifications_test`; private guard/assault assignments and path decisions |
| `src/sim/playtest/military_ai/ground.rs` | `ai_city_defense_playtest` | Unit check: `src/sim/tests/gameplay/military_ai/ground.rs::ai_city_defense_test`; private guard/assault assignments and path decisions |
| `src/sim/playtest/movement_scenarios.rs` | `remembered_route_playtest` | Unit check: `src/sim/tests/gameplay/movement_tests.rs::remembered_route_test`; observed-map path planning and movement-rule edge cases |
| `src/sim/playtest/movement_scenarios.rs` | `nested_cargo_rescue_playtest` | Unit check: `src/sim/tests/gameplay/movement_tests.rs::nested_cargo_rescue_test`; observed-map path planning and movement-rule edge cases |
| `src/sim/playtest/movement_scenarios.rs` | `overseas_assault_playtest` | Unit check: `src/sim/tests/gameplay/movement_tests.rs::overseas_assault_test`; observed-map path planning and movement-rule edge cases |
| `src/sim/playtest/movement_scenarios.rs` | `activity_completion_order_playtest` | Unit check: `src/sim/tests/gameplay/movement_tests.rs::activity_completion_order_test`; observed-map path planning and movement-rule edge cases |
| `src/sim/playtest/movement_scenarios.rs` | `unit_turn_lifecycle_playtest` | Unit check: `src/sim/tests/gameplay/movement_tests.rs::unit_turn_lifecycle_test`; observed-map path planning and movement-rule edge cases |
| `src/sim/playtest/movement_scenarios.rs` | `native_movement_rules_playtest` | Unit check: `src/sim/tests/gameplay/movement_tests.rs::native_movement_rules_test`; observed-map path planning and movement-rule edge cases |
| `src/sim/playtest/movement_scenarios.rs` | `civilian_border_playtest` | Unit check: `src/sim/tests/gameplay/movement_tests.rs::civilian_border_test`; observed-map path planning and movement-rule edge cases |
| `src/sim/playtest/movement_scenarios.rs` | `damaged_movement_playtest` | Unit check: `src/sim/tests/gameplay/movement_tests.rs::damaged_movement_test`; observed-map path planning and movement-rule edge cases |
| `src/sim/playtest/movement_scenarios.rs` | `ai_source_goto_playtest` | Unit check: `src/sim/tests/gameplay/movement_tests.rs::ai_source_goto_test`; observed-map path planning and movement-rule edge cases |
| `src/sim/playtest/naval_trade_scenarios.rs` | `hunter_production_playtest_setup` | Fixture: `src/sim/tests/gameplay/naval_trade_tests.rs::hunter_production_fixture`; hunter/cargo and production fixtures for focused rule tests |
| `src/sim/playtest/naval_trade_scenarios.rs` | `ai_submarine_playtest_setup` | Fixture: `src/sim/tests/gameplay/naval_trade_tests.rs::ai_submarine_fixture`; hunter/cargo and production fixtures for focused rule tests |
| `src/sim/playtest/naval_trade_scenarios.rs` | `submarine_playtest_setup` | Fixture: `src/sim/tests/gameplay/naval_trade_tests.rs::submarine_fixture`; hunter/cargo and production fixtures for focused rule tests |
| `src/sim/playtest/naval_trade_scenarios.rs` | `ai_ferry_registry_playtest` | Unit check: `src/sim/tests/gameplay/naval_trade_tests.rs::ai_ferry_registry_test`; hunter/cargo and production fixtures for focused rule tests |
| `src/sim/playtest/research_scenarios.rs` | `native_unset_research_goal_playtest` | Unit check: `src/sim/tests/gameplay/research_tests.rs::native_unset_research_goal_test`; research sentinels, rounding vectors, cache and save invariants |
| `src/sim/playtest/research_scenarios.rs` | `native_research_arithmetic_playtest` | Unit check: `src/sim/tests/gameplay/research_tests.rs::native_research_arithmetic_test`; research sentinels, rounding vectors, cache and save invariants |
| `src/sim/playtest/research_scenarios.rs` | `ai_research_switching_playtest` | Unit check: `src/sim/tests/gameplay/research_tests.rs::ai_research_switching_test`; research sentinels, rounding vectors, cache and save invariants |
| `src/sim/playtest/research_scenarios.rs` | `native_unset_goal_save_playtest` | Unit check: `src/sim/tests/gameplay/research_tests.rs::native_unset_goal_save_test`; research sentinels, rounding vectors, cache and save invariants |
| `src/sim/playtest/research_scenarios.rs` | `native_research_phase_playtest` | Unit check: `src/sim/tests/gameplay/research_tests.rs::native_research_phase_test`; research sentinels, rounding vectors, cache and save invariants |
| `src/sim/playtest/settlement_scenarios.rs` | `ferry_research_playtest` | Unit check: `src/sim/tests/gameplay/settlement_tests.rs::ferry_research_test`; founder reservations, reachability and advisor pricing rules |
| `src/sim/playtest/settlement_scenarios.rs` | `ai_expansion_playtest` | Unit check: `src/sim/tests/gameplay/settlement_tests.rs::ai_expansion_test`; founder reservations, reachability and advisor pricing rules |
| `src/sim/playtest/settlement_scenarios.rs` | `ai_settlement_goal_government_playtest` | Unit check: `src/sim/tests/gameplay/settlement_tests.rs::ai_settlement_goal_government_test`; founder reservations, reachability and advisor pricing rules |
| `src/sim/playtest/settlement_scenarios.rs` | `ai_settlement_value_playtest` | Unit check: `src/sim/tests/gameplay/settlement_tests.rs::ai_settlement_value_test`; founder reservations, reachability and advisor pricing rules |
| `src/sim/playtest/settlement_scenarios.rs` | `settlement_reservation_order_playtest` | Unit check: `src/sim/tests/gameplay/settlement_tests.rs::settlement_reservation_order_test`; founder reservations, reachability and advisor pricing rules |
| `src/sim/playtest/turn_scenarios.rs` | `government_ui_playtest_setup` | Fixture: `src/sim/tests/gameplay/turn_tests.rs::government_ui_fixture`; exact upkeep accounting and government-transition fixtures |
| `src/sim/playtest/turn_scenarios.rs` | `despotism_upkeep_playtest` | Unit check: `src/sim/tests/gameplay/turn_tests.rs::despotism_upkeep_test`; exact upkeep accounting and government-transition fixtures |
| `src/sim/playtest/unit_action_scenarios.rs` | `cooperative_work_playtest_setup` | Fixture: `src/sim/tests/gameplay/unit_action_tests.rs::cooperative_work_fixture`; unit-action legality, upgrades and experience fixtures |
| `src/sim/playtest/unit_action_scenarios.rs` | `upgrade_playtest_setup` | Fixture: `src/sim/tests/gameplay/unit_action_tests.rs::upgrade_fixture`; unit-action legality, upgrades and experience fixtures |
| `src/sim/playtest/unit_action_scenarios.rs` | `veteran_playtest_setup` | Fixture: `src/sim/tests/gameplay/unit_action_tests.rs::veteran_fixture`; unit-action legality, upgrades and experience fixtures |
| `src/sim/playtest/unit_action_scenarios.rs` | `zone_control_playtest_setup` | Fixture: `src/sim/tests/gameplay/unit_action_tests.rs::zone_control_fixture`; unit-action legality, upgrades and experience fixtures |
| `src/sim/playtest/unit_action_scenarios.rs` | `base_playtest_setup` | Fixture: `src/sim/tests/gameplay/unit_action_tests.rs::base_fixture`; unit-action legality, upgrades and experience fixtures |
| `src/sim/playtest/unit_action_scenarios.rs` | `transport_worker_actions_playtest` | Unit check: `src/sim/tests/gameplay/unit_action_tests.rs::transport_worker_actions_test`; unit-action legality, upgrades and experience fixtures |
| `src/sim/playtest/unit_catalog_scenarios.rs` | `native_unit_catalog_playtest` | Unit check: `src/sim/tests/gameplay/unit_catalog_tests.rs::native_unit_catalog_test`; catalog identity, ordering and imported-value parity |
| `src/sim/playtest/wonder_scenarios.rs` | `obsolete_wonders_playtest` | Unit check: `src/sim/tests/gameplay/wonder_tests.rs::obsolete_wonders_test`; private effect requirements and obsolescence rules |
| `src/sim/playtest/wonder_scenarios.rs` | `theatre_output_playtest` | Unit check: `src/sim/tests/gameplay/wonder_tests.rs::theatre_output_test`; private effect requirements and obsolescence rules |
| `src/sim/playtest/world_events_scenarios.rs` | `run_plague_playtest` | Unit check: `src/sim/tests/gameplay/world_events_tests.rs::run_plague_test`; seeded probability boundaries and literal native RNG oracles |
| `src/sim/playtest/world_events_scenarios.rs` | `run_disaster_playtest` | Unit check: `src/sim/tests/gameplay/world_events_tests.rs::run_disaster_test`; seeded probability boundaries and literal native RNG oracles |
| `src/sim/playtest/world_events_scenarios.rs` | `uprising_playtest` | Unit check: `src/sim/tests/gameplay/world_events_tests.rs::uprising_test`; seeded probability boundaries and literal native RNG oracles |
| `src/sim/research_observation.rs` | `research_history_playtest` | Migrated to `src/scenarios/definitions/research_history.rs`; existing coverage preserved |
| `src/sim/advisors/playtest.rs` | `ai_happiness_effects_playtest` | Unit check: `src/sim/advisors/unit_cases.rs::ai_happiness_effects_test`; private building/defender wants and internal forecast calculations |
| `src/sim/advisors/playtest.rs` | `ai_wonder_advisor_playtest` | Unit check: `src/sim/advisors/unit_cases.rs::ai_wonder_advisor_test`; private building/defender wants and internal forecast calculations |
| `src/sim/ai_economy/playtest.rs` | `ai_tax_manager_playtest` | Unit check: `src/sim/ai_economy/unit_cases.rs::ai_tax_manager_test`; tax manager arithmetic and private emergency/reserve thresholds |
| `src/sim/ai_economy/playtest.rs` | `ai_tax_phase_playtest` | Unit check: `src/sim/ai_economy/unit_cases.rs::ai_tax_phase_test`; tax manager arithmetic and private emergency/reserve thresholds |
| `src/sim/leaders/playtest.rs` | `ai_zero_attack_carrier_path_playtest` | Unit check: `src/sim/leaders/unit_cases.rs::ai_zero_attack_carrier_path_test`; private promotion/spawn odds and leader eligibility |
| `src/sim/settlement_ferries/playtest.rs` | `ai_guard_ferry_capacity_playtest` | Unit check: `src/sim/settlement_ferries/unit_cases.rs::ai_guard_ferry_capacity_test`; private ferry capacity and founder/guard reservations |
| `src/sim/workers/playtest.rs` | `ai_worker_planning_playtest` | Unit check: `src/sim/workers/unit_cases.rs::ai_worker_planning_test`; WorkMap reservations, route selection and shelter decisions |
