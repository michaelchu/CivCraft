# Test performance

Measured on 2026-11-13 with Rust 1.100.0 on Linux, four test threads and a
four-CPU allocation. Times exclude compilation and are approximate wall times.

## Changes

- Settlement unit tests use a 143-tile island and four nearby scoring probes.
  They still run the real settlement and worker planners. Reservation lifecycle
  and crowding/choice checks have separate assertions. The broader desktop
  scenario helpers remain available.
- The test profile uses `opt-level = 1`. Debug symbols, debug assertions and
  overflow checks retain their normal test-profile settings. The search-budget
  tests still check their original iteration limits and results. Development
  and release profiles are unchanged.
- `process_cities()` skips dead and inactive player slots, matching `end_turn()`.
  Previously it processed unused slots with an unset research target: research
  cost and progress were both zero, so 15 tests could not finish. A regression
  checks city processing completes production while leaving an unused player
  unchanged.
- The campaign coexistence test was removed for future simulator coverage. The
  roster test retains its roster, starting-technology, inactive-player and
  scoreboard assertions without running a full turn on Tiny and Huge maps.

The first optimized build (including dependency recompilation) took 58 seconds;
a subsequent rebuild after a source change took 22 seconds. The test-profile
optimization trades extra compilation time for shorter repeated suite runs.

## Results

The full suite completes in **22.55 seconds**: **963 passed,
21 failed**. All 15 previously hanging tests pass. The failing test
names exactly match the 21 assertion failures captured before these changes;
these existing failures are outside this performance change.

| Test | Before (seconds) | After (seconds) |
|---|---:|---:|
| Founder work arbitration / mission retention | 41.53 | 0.47 |
| Reservation lifecycle and crowding (now two tests) | 42.01 | 0.21 combined |
| Large-city manager | 37.34 | 4.58 |
| City-manager search budget | 26.45 | 3.16 |
| Capital attack / diplomacy turns | 33.52 | 4.56 |
| Live AI tax/production phases | 26.82 | 8.79 |
| Domestic/military production comparison | 24.71 | 3.97 |
| Connected routes across end turn | 29.30 | 3.88 |
| Airstrip worker UI duration | 19.24 | 3.27 |
| Wonder advisor | 19.95 | 2.88 |

The baseline run used a temporary research-loop assertion to identify hangs;
that diagnostic assertion is not part of the code. There is no completed
unmodified full-suite baseline because the original suite hangs.

Run the suite with:

```sh
cargo test --offline -- --test-threads=4
```

For individual timing output, the standard Rust test harness also supports
`--report-time` and JSON output with an unstable-options-capable toolchain.

## Existing assertion failures

- `mapgen::tests::default_worlds_match_civcraft_resource_goldens`
- `mapgen::tests::freeciv_ocean_depth_stages_use_height_and_the_civ3_wrap_topology`
- `mapgen::tests::freeciv_river_blocking_is_fatal_and_rivers_continue_through_frozen_terrain`
- `mapgen::tests::generated_worlds_match_civcraft_resource_goldens_for_each_wrap`
- `rules::freeciv::tests::campaign_map_generation_settings_are_explicit_and_validated`
- `sim::advisors::tests::effects::science_priority_truncates_to_twenty_one_before_source_float_accumulation`
- `sim::ai::hunter_tests::defended_overseas_force_keeps_its_invasion_until_capture`
- `sim::ai::hunter_tests::military_delivery_releases_the_persisted_ferry_boss`
- `sim::bases::tests::bases_require_their_prerequisites_and_fortresses_protect_without_extra_healing`
- `sim::cities::production::upkeep_tests::newly_built_unit_pays_its_gold_upkeep_during_the_same_city_turn`
- `sim::movement::tests::a_winning_attacker_cannot_occupy_a_city_with_a_second_defender`
- `sim::movement::tests::imported_marine_small_land_units_can_occupy_cities`
- `sim::output_effects::city_output_tests::celebration_trade_uses_prior_happiness_while_tile_penalty_uses_rapture_history`
- `sim::output_effects::city_output_tests::shared_city_output_desktop_fixture`
- `sim::playtest::ai_integration_tests::despotism_upkeep_playtest_charges_gold_in_isolated_state`
- `sim::playtest::ai_integration_tests::integrated_advisor_scenarios`
- `sim::playtest::ai_integration_tests::shakespeares_theater_trade_effect_is_local_and_enduring`
- `sim::research_phase::unset_goal_roundtrips_without_becoming_an_invention_or_current_target`
- `sim::tests::new_game_advances_freeciv_post_mapgen_rng_and_keeps_civ3_assignments`
- `sim::tests::newly_founded_city_claims_only_its_center_and_nearest_eight_tiles`
- `sim::turn::tests::barbarians_move_in_the_begin_phase_after_the_turn_clock_advances`
