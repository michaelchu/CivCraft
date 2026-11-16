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

## Order of work

1. Migrate building identities, values, requirements, effects, and bindings.
2. Migrate the technology graph and research/economic parameters.
3. Migrate governments, settlement founding/city rules, and settlement AI
   tuning into native TOML.
4. Migrate the remaining unit, building, government, terrain, action, combat,
   culture, diplomacy, AI, victory, and world effects into native TOML.
5. Switch all simulation adapters to native catalogs and remove Freeciv
   gameplay content records from the selected runtime closure.
6. Delete TSV inputs and the conversion tool after a final clean migration and
   full test run.
