# CivCraft native ruleset content

This directory is the CivCraft-owned TOML content layer for the Civilization
III: Conquests campaign.  It is intentionally separate from
`civcraft_civ2civ3/`, whose `.ruleset` files currently provide the retained
Freeciv engine configuration.

The catalogs are the runtime source for the migrated campaign systems.  The
legacy TSV extraction tables remain only as temporary source material for the
conversion tool while building rules and Freeciv effect adapters are being
cut over.  Do not edit the TSVs directly for a rules change once the
corresponding TOML field exists; regenerate only when reconciling source data,
then keep the TOML and focused tests authoritative.

Current catalogs:

- `units.toml`: 92 native unit identities and tuneable source values.
- `experience_levels.toml`: Conquests experience ranks, hit points, and promotion values.
- `civilizations.toml`: 31 civilizations, city lists, starting technologies,
  and starting-unit profiles.
- `buildings.toml`: 83 source building records, 91 stable runtime rules, and
  explicit legacy binding status.
- `technologies.toml`: 83 native Conquests advances, source coordinates, costs,
  prerequisite graph inputs, and native capability flags (including the Civ
  III Engineering bridge capability).
- `effects.toml`: the complete 508-effect campaign graph, including typed
  requirements and multipliers.
- `actions.toml`: the 19 campaign actions and 89 typed action-enabler
  branches.
- `governments.toml`: the eight Conquests governments, names, prerequisites,
  AI preferences, flags, core scalar values, and support/assimilation/
  resistance/military-law tables.
- `economy.toml`: city growth, output, upkeep, and founding parameters.
- `settlements.toml`: settlement AI, citizen nationality/partisan rules, and
  village outcome parameters.
- `terrain.toml`: 14 native terrain identities, movement/activity values,
  worker results, native classes, flags, climate transitions, and terrain
  parameters.

The campaign loader replaces the selected ruleset's effect records with this
native graph and resolves technology requirements through the native
technology catalog. The remaining `.ruleset` files are retained as parser and
engine-schema inputs for still-unmigrated generic engine tables; they are not
the source for migrated unit, technology, government, civilization, effect, or
native terrain data.

Unit shield costs are authoritative in `units.toml` (`cost_shields`). Production,
settlement AI valuation, and shield-recovery base values share that cost.
The unit-economics crosswalk has been removed. Capabilities are derived from
native PRTO flags; experience and government support use their native catalogs.
Barbarian and partisan choices are configured by native IDs in `settlements.toml`.
The generic engine's remaining algorithm gaps are recorded in
`docs/CIV3_MANUAL_RULES.md`; removal does not claim full gameplay parity.
