# Native TOML ruleset migration goal

Make `civcraft` a self-owned, Civ III: Conquests-native campaign.  Gameplay
content must come from editable TOML under `data/rulesets/civcraft_native/`;
Freeciv code may remain as an engine/schema implementation detail, but no
Freeciv units, buildings, governments, technologies, nations, or settlement
records may be selected as gameplay content.

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
- `unit_economics.tsv`, the remaining Conquests TSV catalogs, and the imported
  Freeciv content records are removed only after their TOML replacements are
  live and the full test suite is green.
- Existing save IDs and player-facing civilization labels remain compatible;
  any intentional identity change gets an explicit save migration.

## Implemented slice

Native TOML now contains the complete 92-unit, 31-civilization, 83-source /
91-runtime-rule building, and 83-technology Conquests catalogs, plus the
eight-government tables and
native economy/settlement settings.  The source-content catalog and migrated
unit consumers now load TOML directly.  Unit upkeep, roles, flags, build cost,
classes, targetability, and action requirements now prefer native values;
unit upkeep still retains a compatibility adapter for effect evaluation.
Building identities and rules,
government identity/selection/prerequisites, government flags, and core
government scalar values now load from TOML; broader building and government
effect graphs still need a deliberate runtime cutover.

The native catalog also now owns the complete selected C2C3 effect graph. The
campaign loader converts `effects.toml` into the reusable engine effect model,
and technology requirements bind directly to the native 83-advance catalog or
an explicit CivCraft compatibility mapping. Multiline class flags and role
lists are preserved by the migration tooling.

The selected action graph is now native as well: `actions.toml` owns all 19
actions and 89 actor/target enabler branches, while the engine continues to
evaluate their typed requirements.

Native terrain is now live as well. `terrain.toml` owns the selected terrain
movement/activity values, output values, native classes, flags, climate
transitions, terrain parameters, and extra-removal settings. City output,
climate changes, and the normal movement adapter read that catalog directly;
the legacy ruleset terrain path remains only for isolated parser/mutation parity
tests.

The final unit-runtime cutover is now in place. Native unit economics owns
vision radius/layer, non-native hit-point attrition, unit-class flags, and the
remaining combat/transport capability lookups. Building flags, technology
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
