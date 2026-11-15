# Native TOML ruleset migration goal

Make `civcraft` a self-owned, Civ III: Conquests-native campaign.  Gameplay
content must come from editable TOML under `data/rulesets/civcraft_native/`;
Freeciv code may remain as an engine/schema implementation detail, but no
Freeciv units, buildings, governments, technologies, nations, or settlement
records may be selected as gameplay content.

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

`units.toml` and `civilizations.toml` now contain the complete 92-unit and
31-civilization Conquests catalogs, including civilization starting units and
the tuneable unit fields currently used by simulation.  A native loader and
parity tests are in place beside the TSV-backed adapters.  This is a migration
boundary, not yet the runtime cutover.

## Order of work

1. Migrate buildings and their bindings.
2. Migrate the technology graph and research/economic parameters.
3. Migrate governments, settlement founding/city rules, and settlement AI
   tuning into native TOML.
4. Switch simulation adapters to native catalogs and remove Freeciv content
   records from the selected runtime closure.
5. Delete TSV inputs and the conversion tool after a final clean migration and
   full test run.
