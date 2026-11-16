# Native TOML ruleset migration goal

Make `civcraft` a self-owned, Civ III: Conquests-native campaign.  Gameplay
content must come from editable TOML under `data/rulesets/civcraft/content/`;
Freeciv code may remain as an engine/schema implementation detail, but no
Freeciv units, buildings, governments, technologies, nations, or settlement
records may be selected as gameplay content.

## Unit authority

Civilization III: Conquests is the authority for unit statistics. Native
`units.toml` is the single editable source for those values; production, AI
valuation, and shield recovery use its shield cost without Freeciv shieldbox
scaling. The BIQ provenance is recorded on each unit and in
`CIV3_UNIT_ROSTER_RECONCILIATION.md`.

The unit-economics TOML/TSV crosswalks have been removed. Native PRTO data
supplies unit identity, statistics, capacity, action flags, class, sight,
and support eligibility; the native government and experience catalogs supply
support allowances and rank values. `sim/native_unit_rules.rs` derives the
requirement evaluator's capability vocabulary without selecting foreign units.
Barbarian and partisan unit choices are explicit native settlement policy.

The generic action/effect, movement, aircraft-planning, and recovery algorithms
remain migration work. Removing the crosswalk does not establish Civ III parity
for these algorithms. See `CIV3_MANUAL_RULES.md`'s native-unit cutover record.

## Completion goal

Finish the migration so the native TOML catalog is the authoritative runtime
source for every gameplay rule. The completed migration must:

- remove runtime dependencies on Freeciv unit, building, government,
  civilization, nation, and technology content records;
- replace the remaining Freeciv effect, action, terrain, economy, settlement,
  combat, culture, diplomacy, AI, and victory bindings with native TOML data or
  explicitly engine-owned algorithms;
- migrate the remaining unit classes, roles, flags, targetability, vision,
  combat modifiers, and special abilities into native unit rules;
- make building effects and all government support/effect tables authoritative
  from native TOML;
- keep stable save identifiers and provide explicit migrations for any removed
  compatibility identities; and
- delete obsolete TSV catalogs and the migration converter once parity tests,
  documentation, and source manifests no longer require them.

The migration is complete only when a normal default build and test run do not
load Freeciv gameplay content, native TOML validation and parity tests pass,
and the remaining Freeciv module is limited to reusable engine algorithms and
parser infrastructure.

## Acceptance criteria

- Units, buildings, technologies, governments, civilizations, settlement
  rules, and economy settings have native TOML schemas with stable IDs.
- Runtime loaders consume those TOML catalogs directly.
- Focused parity tests protect the current Conquests identity and values while
  each catalog is migrated.
- Remaining Conquests TSV catalogs and imported Freeciv content records are
  removed only after their native replacements are live and the full test
  suite is green. The unit-economics TOML/TSV crosswalks are already removed.
- Existing save IDs and player-facing civilization labels remain compatible;
  any intentional identity change gets an explicit save migration.

## Implemented slice

Native TOML now contains the complete 92-unit, 31-civilization, 83-source /
91-runtime-rule building, and 83-technology Conquests catalogs, plus the
eight-government tables and
native economy/settlement settings.  The source-content catalog and migrated
unit consumers now load TOML directly.  Unit upkeep, roles, flags, build cost,
classes, targetability, and action requirements now read native values.
Unit treasury support uses the native government allowances nationally.
Building identities and rules,
government identity/selection/prerequisites, government flags, and core
government scalar values now load from TOML; broader building and government
effect graphs still need a deliberate runtime cutover.

The native catalog also now owns the complete selected C2C3 effect graph. The
campaign loader converts `effects.toml` into the reusable engine effect model,
and technology requirements bind directly to the native 83-advance catalog or
an explicit CivCraft compatibility mapping. Foreign unit class-flag and role lists are no longer imported.

The selected action graph is now native as well: `actions.toml` owns all 19
actions and 89 actor/target enabler branches, while the engine continues to
evaluate their typed requirements.

Native terrain is now live as well. `terrain.toml` owns the selected terrain
movement/activity values, output values, native classes, flags, climate
transitions, terrain parameters, and extra-removal settings. City output,
climate changes, and the normal movement adapter read that catalog directly;
the legacy ruleset terrain path remains only for isolated parser/mutation parity
tests.

The unit-economics crosswalk is removed. Native PRTO flags now supply
vision, unit-class capabilities, and transport restrictions. Unsupported
foreign per-target combat bonuses and percentage class attrition are absent;
experience power/movement modifiers are neutral. Treasury support uses native
government allowances and PRTO support eligibility, without home-city food or
shield upkeep. Serialized unit IDs and compatibility state fields are retained;
no save identity migration is required. Building flags, technology
capabilities, and settlement citizen rules are native TOML fields as well.
Worker extras still use the retained Freeciv requirement evaluator as an
engine algorithm, but their selected action graph, terrain prerequisites,
native technology capability, and runtime tile state are CivCraft-owned.

## Order of work

1. Finish the remaining generic engine-table adapters whose values are still
   intentionally supplied by selected `.ruleset` files.
2. Switch those adapters to native catalogs or explicitly engine-owned
   algorithms, keeping Freeciv records out of the selected gameplay closure.
3. Delete TSV inputs and the conversion tool after the final clean migration
   and full test run; retain only source manifests that are still required for
   reproducible catalog generation.

## Unit-crosswalk removal validation

- `cargo test --offline --all-targets`: 1,078 library tests plus five other
  tests passed; one library test remains ignored.
- `cargo test --offline --doc`: passed (no documentation tests defined).
- Headless `settlement-ferry`, one turn: all three scenario checks passed,
  including founding the overseas city.
- The converter no longer reads or emits a unit-economics crosswalk. Native
  unit TOML rejects unknown fields; settlement policy unit IDs are validated.
