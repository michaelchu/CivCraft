# CivCraft campaign ruleset

This directory is the single CivCraft campaign ruleset package for the
Civilization III: Conquests campaign. Its editable layers are `content/`, which
contains CivCraft-owned native TOML catalogs, while `engine/` contains the
Freeciv-compatible inputs still needed by the reusable rules engine.

The catalogs under `content/` are the runtime source for the migrated campaign systems.  The
legacy extraction tables and one-time conversion tool have been removed after
the generated TOML catalogs were verified against them.  Edit the TOML files
and keep the focused catalog and parity tests authoritative; campaign content is now independent of BIQ records.
See [the configuration contract](../../../docs/RULESET_CONFIGURATION.md).

Current catalogs:

- `content/units.toml`: 92 native unit identities and tuneable source values.
- `content/experience_levels.toml`: Conquests experience ranks, hit points, and promotion values.
- `content/civilizations.toml`: 31 civilizations, city lists, starting technologies,
  and starting-unit profiles.
- `content/buildings.toml`: 91 active rules with stable IDs, named requirements,
  and `selected_obsolescence` policy flags.
- `content/technologies.toml`: 85 native technology identities, costs,
  prerequisites, eras, and research metadata.
- `content/governments.toml`: the eight Conquests governments, names, prerequisites,
  AI preferences, flags, and support/assimilation/
  resistance/military-law tables.
- `content/economy.toml`: city growth, output, upkeep, and founding parameters.
- `content/settlements.toml`: settlement AI, citizen nationality/partisan rules, and
  village outcome parameters.
- `content/strategy.toml`, `content/diplomacy.toml`, `content/vassalage.toml`,
  `content/religion.toml`: AI strategy, diplomacy, vassalage, and religion rules.

The campaign embeds its engine ruleset and native catalogs when built. The
effect graph, including government rate caps, empire size and civil-war chance, comes from
`engine/effects.ruleset`, and action enablers come from
`engine/actions.ruleset`. Terrain yields, movement, activity timing, and climate
live in `src/rules/terrain_catalog.rs`, while `engine/terrain.ruleset` retains
extras, resources, and movement parameters.

Unit shield costs are authoritative in `content/units.toml` (`cost_shields`). Production,
settlement AI valuation, and shield-recovery base values share that cost.
The unit-economics crosswalk has been removed. Capabilities and AI roles use named fields rather than raw PRTO masks; experience and government support use their native catalogs.
Barbarian and partisan choices are configured by native IDs in
`content/settlements.toml`.
The generic engine's remaining algorithm gaps are recorded in the Freeciv port
plan; removal of inherited content does not claim full gameplay parity.

Animation pacing lives in `data/presentation/unit_animation_pacing.toml`; it does not define simulation behavior. BIQ files and extracted metadata are no longer required.
