# Great Leader Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the existing `Leader` unit a Civ III Great Leader: never buildable, created by elite combat victories, and able to complete a city's current building production.

**Architecture:** Extend the existing `UnitKind::Leader` in place. Creation is a roll in the combat/promotion path; the odds live in `settlements.toml`. The unchecked `help_wonder` action becomes a validated `complete_production` command shared by the human UI and a small AI behavior. No Freeciv code is removed: the Leader was never a `GameLoss` unit.

**Tech Stack:** Rust, `cargo nextest`, native TOML rulesets under `data/rulesets/civcraft/content/`, headless simulator scenarios.

**Spec:** `docs/superpowers/specs/2026-11-17-great-leader-design.md`

## Global Constraints

- Leader creation and use follow Civ III as an explicit, owner-approved exception to the Freeciv-authority rule in `AGENTS.md`; record it in `AGENTS.md` and `docs/FREECIV_GAMEPLAY_PORT_PLAN.md`.
- Odds per eligible kill: attacking 1 in 16 (1 in 12 with Heroic Epic); defending 1 in 32 (1 in 24 with Heroic Epic). Values live in data, not code.
- A Leader can never be built, queued or bought from any path (city screen, worklists, AI choice).
- Leaders complete improvements, small wonders and Great Wonders only. Coinage (convert genus), spaceship parts and units are excluded.
- A Leader never ends a game, triggers civil war, or can be captured; it can be killed.
- One living Leader per owner. A unit that created a Leader never creates another.
- Rolling for a Leader must not draw from the RNG when the combat is ineligible.
- Heroic Epic (200-shield small wonder, better Leader odds) requires the owner to have at least one elite unit (top veteran level); it has no tech requirement.
- Old saves must load: new unit field defaults to false.
- Headless-only builds (`--no-default-features --features scenarios`) must keep compiling with no graphics crates.

## Review Focus

- A Leader alone on a tile that an enemy enters is destroyed, never transferred (Task 1).
- Production changed after a Leader completes it must not move the stock to another item (Task 6).
- Heroic Epic stays unbuildable for an owner with no elite unit and becomes buildable the turn one reaches elite (Task 5).
- A save written before this change loads with `created_leader == false` (Task 3).
- Ineligible combats consume no RNG, so seeded goldens do not shift (Task 4).
- An AI Leader with no legal target stays in its city and never wanders into danger or is assigned as a defender (Task 7).

---

### Task 1: Leader is unbuildable and uncapturable

**Files:**
- Modify: `src/sim/civ3_unit_rules.rs` (flag derivation near `F::NoBuild` and `F::Capturable`)
- Test: `src/sim/cities/tests.rs`

**Interfaces:**
- Produces: `UnitKind::Leader.has_flag(UnitFlag::NoBuild) == true`; `UnitFlag::Capturable` false for Leader.

- [ ] **Step 1: Write failing tests** in `src/sim/cities/tests.rs`:
  `leader_can_never_be_produced_or_queued` loops every `crate::rules::civilizations::all()` entry (set `game.players[1].name` to its name, create a city for owner 1, give the player every tech) and asserts `!game.can_produce(&city, Production::Unit(UnitKind::Leader))`; also assert the city production view built in `src/sim/client_views/city.rs` reports the Leader with `available == false` and `can_queue == false`.
  `leader_is_not_capturable` asserts `!UnitKind::Leader.has_flag(UnitFlag::Capturable)`.
  `enemy_entering_a_lone_leader_destroys_it`: place an owner-1 Leader alone on a tile, move an at-war owner-2 land unit onto it with the same movement command the existing capture tests use; assert no unit of kind `Leader` exists for either owner afterwards.
- [ ] **Step 2: Run** `cargo nextest run --profile ci leader_` — expect FAIL on all three.
- [ ] **Step 3: Implement** in `civ3_unit_rules.rs`: add `|| self.id == "leader"` to the `NoBuild` condition and `&& self.id != "leader"` to the `Capturable` condition.
- [ ] **Step 4: Run** the same command — expect PASS. Then `cargo nextest run --profile ci --all-targets` — expect no regressions.
- [ ] **Step 5: Commit** `git commit -m "Make Leaders unbuildable and uncapturable"` (add Co-Authored-By trailer per repo convention).

### Task 2: Great Leader odds in data

**Files:**
- Modify: `data/rulesets/civcraft/content/settlements.toml`, `src/rules/native.rs` (`SettlementCatalog`), `src/rules/engine_rules.rs` (re-export)
- Create: `src/rules/engine_rules/great_leader.rs`
- Test: in `src/rules/engine_rules/great_leader.rs`

**Interfaces:**
- Produces: `GreatLeaderSettings::campaign() -> &'static GreatLeaderSettings` with `pub(crate) attack_denominator`, `attack_heroic_epic_denominator`, `defense_denominator`, `defense_heroic_epic_denominator` (all `i32`, each > 0).

- [ ] **Step 1: Write failing test** `great_leader_odds_match_the_design` asserting the four values are 16, 12, 32, 24.
- [ ] **Step 2: Run** `cargo nextest run --profile ci great_leader_odds` — expect FAIL (not defined).
- [ ] **Step 3: Implement** a `[great_leader]` table in `settlements.toml` with those four keys; a `NativeGreatLeaderRules` struct and `great_leader` field on `SettlementCatalog`; `GreatLeaderSettings::campaign()` following the `SettlementAiSettings::campaign()` pattern (validate every value > 0 with `assert!`).
- [ ] **Step 4: Run** the test — expect PASS.
- [ ] **Step 5: Commit** `git commit -m "Add Great Leader odds to native settlement rules"`.

### Task 3: `created_leader` unit field and save format

**Files:**
- Modify: `src/sim/model/entities.rs` (`Unit`, its constructor), `src/sim/save/schema/records.rs` (`UnitWire`)
- Test: `src/sim/save/tests.rs`

**Interfaces:**
- Produces: `Unit.created_leader: bool` (default `false`).

- [ ] **Step 1: Write failing tests** `created_leader_round_trips_through_save` (set the flag on a unit, `save_campaign`, `load_campaign`, assert true) and `save_without_created_leader_loads_false` (deserialize a unit record JSON/bincode form lacking the field via the same path existing back-compat tests use; assert false).
- [ ] **Step 2: Run** `cargo nextest run --profile ci created_leader` — expect FAIL.
- [ ] **Step 3: Implement** the field on `Unit` (default false in `Unit::new`) and on `UnitWire` with `#[serde(default)]`; bump the save schema version only if existing convention in `src/sim/save` requires it for added fields, and mirror how `c2c3_veteran` is handled.
- [ ] **Step 4: Run** the tests plus `cargo nextest run --profile ci save` — expect PASS.
- [ ] **Step 5: Commit** `git commit -m "Track whether a unit has created a Leader"`.

### Task 4: Great Leader creation

**Files:**
- Create: `src/sim/great_leader.rs` (declare in `src/sim/mod.rs`)
- Modify: `src/sim/movement/combat.rs` (call site after `promote_combat_veterans`, around line 942), `src/sim/lifecycle.rs` (`UnitOrigin`), `docs/HEADLESS_ANALYTICS.md`
- Test: `src/sim/great_leader.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `GreatLeaderSettings::campaign()` (Task 2), `Unit.created_leader` (Task 3), `selected_veteran_system(kind)` and `Unit.c2c3_veteran` in `src/sim/veterancy.rs`.
- Produces:
  - `Simulation::great_leader_eligible(&self, winner: &Unit, loser: &Unit) -> bool` (no RNG).
  - `Simulation::roll_great_leader(&mut self, winner_id: u32, loser: &Unit, winner_attacked: bool) -> Option<u32>` returning the new Leader's id.
  - `lifecycle::UnitOrigin::GreatLeader { winner_id: u32 }` (serialized as source `great_leader`).

- [ ] **Step 1: Write failing tests** in `great_leader.rs`: `eligibility_requires_an_elite_land_winner` (table: elite land winner and real loser → true; veteran-level winner, naval winner, air winner, barbarian loser, winner with `transport` set to an Army, `created_leader == true`, or an owner that already has a living Leader → each false); `ineligible_combat_draws_no_rng` (compare the RNG state with the accessor the determinism tests in `src/sim/trace/mod.rs` use, before and after `roll_great_leader` on an ineligible pair; assert equal); `roll_eventually_creates_one_leader_on_the_winners_tile` (loop up to 2000 rolls with a seeded game; assert exactly one new `Leader` of the winner's owner at the winner's tile, the winner's `created_leader` is true, and further rolls create none); `heroic_epic_uses_the_better_odds` (assert the selected denominator is `attack_heroic_epic_denominator` when the owner has `Building::HEROIC_EPIC` in any city; expose the choice as a private `fn great_leader_denominator(&self, owner: usize, attacked: bool) -> i32` and test that); `created_leader_is_recorded_in_lifecycle` (the unit_created change has origin `great_leader`).
- [ ] **Step 2: Run** `cargo nextest run --profile ci great_leader` — expect FAIL.
- [ ] **Step 3: Implement** `great_leader_eligible` (top level = `c2c3_veteran + 1 == system.levels.len()`; land domain; no `no_veteran()`), the roll (`rand_below(denominator) == 0`, only after eligibility passes, spawn via `spawn_unit_with_origin`), and the `UnitOrigin` variant. Hook it in `combat.rs`: when `def_hp <= 0` call it for the attacker as winner with `winner_attacked = true`; when `att_hp <= 0` call it for the defender as winner with `winner_attacked = false`, passing the pre-removal clone of the loser. Document the new origin in `docs/HEADLESS_ANALYTICS.md`.
- [ ] **Step 4: Run** `cargo nextest run --profile ci --all-targets` — expect PASS with no seeded-golden changes.
- [ ] **Step 5: Commit** `git commit -m "Create Great Leaders from elite victories"`.

### Task 5: Heroic Epic requires an elite unit

**Files:**
- Modify: `src/rules/buildings.rs` (`Requirement` enum and `native_building_requirement`), `src/sim/buildings.rs` (`building_requirement_met`, the requirement label near line 389), `data/rulesets/civcraft/content/buildings.toml` (Heroic Epic entry), `tests/fixtures/building_rules_v1.json` (the Heroic Epic baseline entry, changed deliberately)
- Test: `src/sim/buildings.rs` or the existing building-rule tests

**Interfaces:**
- Consumes: `great_leader_eligible`'s top-level test from Task 4; factor it into `Simulation::owner_has_elite_unit(&self, owner: usize) -> bool` (any living unit of `owner` with a veteran system at its top level) in `src/sim/great_leader.rs`.
- Produces: `Requirement::OwnerEliteUnit` (TOML `kind = "OwnerEliteUnit"`, no value), shown in blocker text as "an elite unit".

- [ ] **Step 1: Write failing tests** `heroic_epic_needs_an_elite_unit` (a new city for an owner with no elite unit: `building_blocker(&city, Building::HEROIC_EPIC)` is `Some`; promote one of the owner's units to its top `c2c3_veteran` level: blocker is `None` and `can_produce` is true) and `owner_has_elite_unit_ignores_other_owners_and_civilians`.
- [ ] **Step 2: Run** `cargo nextest run --profile ci heroic_epic owner_has_elite` — expect FAIL.
- [ ] **Step 3: Implement** the new `Requirement` variant, its parse arm and evaluation (`self.owner_has_elite_unit(city.owner) == present`), the label, and set Heroic Epic's `requirements` to `[{ kind = "OwnerEliteUnit", present = true }]`. Update only the Heroic Epic entry in `tests/fixtures/building_rules_v1.json` so `named_references_preserve_pre_migration_building_rules_and_save_ids` passes, and say so in the commit message.
- [ ] **Step 4: Run** the new tests and `cargo nextest run --profile ci --all-targets` — expect PASS.
- [ ] **Step 5: Commit** `git commit -m "Require an elite unit to build Heroic Epic"`.

### Task 6: Complete production command

**Files:**
- Modify: `src/sim/leaders.rs` (`can_help_wonder`, `help_wonder`), `src/sim/commands.rs` (`Command::HelpWonder`), `src/sim/client_views/units.rs`, `src/sim/client_views/mod.rs` (`can_help_wonder` field), `src/client/actions.rs`, `src/client/unit_actions.rs`, `src/client/mod.rs` (`UiAction::HelpWonder`)
- Test: `src/sim/cities/tests.rs`

**Interfaces:**
- Produces: `Simulation::can_complete_production(&self, unit: &Unit, city: &City) -> bool`; `Simulation::complete_production(&mut self, unit_id: u32, city_id: u32) -> bool`; `Command::CompleteProduction { unit_id: u32, city_id: u32 }`; `UiAction::CompleteProduction(u32)`; view field `can_complete_production`. Replace every use of the old names; no aliases.

- [ ] **Step 1: Write failing tests** `leader_completes_only_legal_buildings`: Leader in its owner's city completes Pyramids and Temple (stock becomes cost, Leader removed, `bought_production` true); is refused for Coinage, a spaceship part, a unit, an already-built building, another owner's city, and a city with `bought_production` already set. `completed_production_cannot_be_redirected`: after completion `can_change_production(city, other)` is false and the owner's gold is unchanged after `process_player_cities`. `completed_item_finishes_at_turn_end`: run `process_player_cities`; the building is present.
- [ ] **Step 2: Run** `cargo nextest run --profile ci leader_completes completed_` — expect FAIL.
- [ ] **Step 3: Implement** per the Global Constraints: require unit kind `Leader`, `moves_left > 0`, same owner and tile as the city, `Production::Building(b)` with genus `Improvement`, `SmallWonder` or `GreatWonder`, `self.can_produce(city, production)`, stock below cost and `!city.bought_production`. On use set stock to `production.cost()`, set `bought_production = true`, remove the unit with `UnitRemoval::Expended`, add the event "A Leader completed production in {city}." Rename the command, UI action, label ("Complete production: {city}") and view field.
- [ ] **Step 4: Run** the tests, then `cargo nextest run --profile ci --all-targets` — expect PASS.
- [ ] **Step 5: Commit** `git commit -m "Let Leaders complete improvements and wonders"`.

### Task 7: AI Leader behavior

**Files:**
- Create: `src/sim/ai/great_leader.rs` (declare in `src/sim/ai.rs`)
- Modify: `src/sim/ai.rs` (unit loop, before the generic military handling)
- Test: `src/sim/ai/great_leader.rs` (`#[cfg(test)]`)

**Interfaces:**
- Consumes: `complete_production` (Task 6), `ai_wonder_city`, `ai_goto_unit` in `src/sim/normal_paths.rs`.
- Produces: `Simulation::great_leader_target(&self, unit: &Unit) -> Option<u32>` (city id); `Simulation::run_ai_great_leader(&mut self, unit: &Unit) -> bool` (true when it consumed the unit's turn).

- [ ] **Step 1: Write failing tests** `target_prefers_a_wonder_in_the_wonder_city`, `target_falls_back_to_the_most_expensive_building_then_nearest`, `no_legal_target_means_no_target`, `leader_in_its_target_city_completes_production`, and `leader_without_target_stays_put_and_is_never_a_defender` (run the AI unit pass; assert the Leader's tile is unchanged and it is absent from the AI's city-defender and guard assignment maps).
- [ ] **Step 2: Run** `cargo nextest run --profile ci great_leader_ai` (name the tests with that prefix) — expect FAIL.
- [ ] **Step 3: Implement** the target policy from the spec (wonder city first if it builds a legal wonder, otherwise the owner's city with the most expensive legal building, ties to the nearest by `map_distance`); move with `ai_goto_unit` using the existing task variant for civilian transit; call `complete_production` on arrival; otherwise fortify. Dispatch it from the `ai.rs` unit loop for `UnitKind::Leader` ahead of defender and guard handling.
- [ ] **Step 4: Run** the tests and the full suite — expect PASS.
- [ ] **Step 5: Commit** `git commit -m "Teach the AI to use Great Leaders"`.

### Task 8: Registered scenario, docs, verification

**Files:**
- Modify: `src/sim/scenarios/regression/headless.rs` (register `great-leader`), add `Simulation::great_leader_playtest(&mut self) -> (bool, bool, bool)` beside the other `*_playtest` helpers, `AGENTS.md`, `docs/FREECIV_GAMEPLAY_PORT_PLAN.md`, `README.md` if it lists scenarios

**Interfaces:**
- Consumes: Tasks 1, 4, 5, 6, 7.

- [ ] **Step 1: Write the scenario** `great_leader` with checks `leader_unbuildable`, `leader_completes_wonder`, `leader_consumed`, built from the playtest helper (an AI civ with a Leader in a city building a wonder, one turn simulated).
- [ ] **Step 2: Run** `cargo run --offline --release --bin headless_sim -- --scenario great-leader --turns 1 --report /tmp/gl.json`; expect exit 0 and `summary.scenario_checks_passed == scenario_checks_total == 3`.
- [ ] **Step 3: Document** the Civ III exception for Leader creation and use in `AGENTS.md` and `docs/FREECIV_GAMEPLAY_PORT_PLAN.md`, stating that the Leader is not a `GameLoss` unit.
- [ ] **Step 4: Verify everything**: `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`; `cargo nextest run --profile ci --all-targets`; `cargo test --doc`; `cargo build --locked --no-default-features --features scenarios --bin headless_sim`. Then run seed 42 for 150 turns and read `summary.lifecycle.unit_totals` for `Leader` (created, existing, used) to confirm Leaders appear in a real game; report the counts rather than asserting a number.
- [ ] **Step 5: Commit** `git commit -m "Add the great-leader scenario and document the Civ III exception"`.
