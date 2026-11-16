# CivCraft campaign ruleset

This directory is the single CivCraft campaign ruleset package for the
Civilization III: Conquests campaign. It has two explicit layers: `content/`
contains CivCraft-owned native TOML catalogs, while `engine/` contains the
Freeciv-compatible inputs still needed by the reusable rules engine.

The catalogs under `content/` are the runtime source for the migrated campaign systems.  The
legacy extraction tables and one-time conversion tool have been removed after
the generated TOML catalogs were verified against them.  Edit the TOML files
and keep the focused catalog and parity tests authoritative; source BIQ
provenance remains recorded in the catalog records and migration documents.

Current catalogs:

- `content/units.toml`: 92 native unit identities and tuneable source values.
- `content/experience_levels.toml`: Conquests experience ranks, hit points, and promotion values.
- `content/civilizations.toml`: 31 civilizations, city lists, starting technologies,
  and starting-unit profiles.
- `content/buildings.toml`: 83 source building records, 91 stable runtime rules, and
  explicit legacy binding status.
- `content/technologies.toml`: 83 native Conquests advances, source coordinates, costs,
  prerequisite graph inputs, and native capability flags (including the Civ
  III Engineering bridge capability).
- `content/effects.toml`: the complete 508-effect campaign graph, including typed
  requirements and multipliers.
- `content/actions.toml`: the 19 campaign actions and 89 typed action-enabler
  branches.
- `content/governments.toml`: the eight Conquests governments, names, prerequisites,
  AI preferences, flags, core scalar values, and support/assimilation/
  resistance/military-law tables.
- `content/economy.toml`: city growth, output, upkeep, and founding parameters.
- `content/settlements.toml`: settlement AI, citizen nationality/partisan rules, and
  village outcome parameters.
- `content/terrain.toml`: 14 native terrain identities, movement/activity values,
  worker results, native classes, flags, climate transitions, and terrain
  parameters.

The campaign loader opens `engine/` and overlays the native effect, action,
technology, and source-content bindings from `content/`. The `.ruleset` files
are not a second campaign; they are the compatibility layer for generic engine
tables that have not yet been migrated to native schemas.

Unit shield costs are authoritative in `content/units.toml` (`cost_shields`). Production,
settlement AI valuation, and shield-recovery base values share that cost.
The unit-economics crosswalk has been removed. Capabilities are derived from
native PRTO flags; experience and government support use their native catalogs.
Barbarian and partisan choices are configured by native IDs in
`content/settlements.toml`.
The generic engine's remaining algorithm gaps are recorded in the Freeciv port
plan; removal of inherited content does not claim full gameplay parity.
